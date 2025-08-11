// Copyright (c) 2025 vivo Mobile Communication Co., Ltd.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//       http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::thread::OsThread;
use core::{
    ffi,
    ptr::NonNull,
    sync::atomic::{compiler_fence, Ordering},
};

use blueos::{
    error::{code, Error},
    scheduler,
    sync::event_flags::{EventFlags, EventFlagsMode},
    thread, time,
};
use cmsis_os::*;
use log;

const SIGNAL_ERROR: ffi::c_int = 0x80000000u32 as ffi::c_int;
// Set the specified Signal Flags of an active thread.
// \param[in]     thread_id     thread ID obtained by \ref osThreadCreate or \ref osThreadGetId.
// \param[in]     signals       specifies the signal flags of the thread that should be set.
// \return previous signal flags of the specified thread or 0x80000000 in case of incorrect parameters.
// int32_t osSignalSet (osThreadId thread_id, int32_t signals);
#[no_mangle]
pub extern "C" fn osSignalSet(thread_id: osThreadId, signals: ffi::c_int) -> ffi::c_int {
    if thread_id.is_null() {
        return SIGNAL_ERROR;
    }
    let thread = unsafe { &*(thread_id as *const _ as *const OsThread) };
    match thread.set_event_flags(signals as u32) {
        Ok(prev_flags) => prev_flags as ffi::c_int,
        Err(e) => SIGNAL_ERROR,
    }
}

// Clear the specified Signal Flags of an active thread.
// \param[in]     thread_id     thread ID obtained by \ref osThreadCreate or \ref osThreadGetId.
// \param[in]     signals       specifies the signal flags of the thread that shall be cleared.
// \return previous signal flags of the specified thread or 0x80000000 in case of incorrect parameters or call from ISR.
// int32_t osSignalClear (osThreadId thread_id, int32_t signals);
#[no_mangle]
pub extern "C" fn osSignalClear(thread_id: osThreadId, signals: ffi::c_int) -> ffi::c_int {
    if thread_id.is_null() {
        return SIGNAL_ERROR;
    }
    let thread = unsafe { &*(thread_id as *const _ as *const OsThread) };
    thread.clear_event_flags(signals as u32) as ffi::c_int
}

// Wait for one or more Signal Flags to become signaled for the current \b RUNNING thread.
// \param[in]     signals       wait until all specified signal flags set or 0 for any single signal flag.
// \param[in]     millisec      \ref CMSIS_RTOS_TimeOutValue or 0 in case of no time-out.
// \return event flag information or error code.
// osEvent __osSignalWait (int32_t signals, uint32_t millisec);
#[no_mangle]
pub extern "C" fn __osSignalWait(signals: ffi::c_int, millisec: u32) -> osEvent {
    let thread = scheduler::current_thread();

    let result = if let Some(alien_ptr) = thread.get_alien_ptr() {
        let cmsis_thread = unsafe { &*(alien_ptr.as_ptr() as *mut OsThread) };
        if (signals != 0) {
            cmsis_thread.wait_event_flags(
                signals as u32,
                EventFlagsMode::ALL,
                time::tick_from_millisecond(millisec as usize),
            )
        } else {
            cmsis_thread.wait_event_flags(
                signals as u32,
                EventFlagsMode::ANY,
                time::tick_from_millisecond(millisec as usize),
            )
        }
    } else {
        log::warn!("not a cmsis thread");
        return osEvent {
            status: osStatus_osErrorOS,
            value: osEvent__bindgen_ty_1 { v: 0 },
            def: osEvent__bindgen_ty_2 {
                message_id: core::ptr::null_mut(),
            },
        };
    };

    osEvent {
        status: match result {
            Ok(_flags) => osStatus_osEventSignal,
            Err(e) => {
                if e == code::ETIMEDOUT {
                    osStatus_osEventTimeout
                } else {
                    osStatus_osErrorOS
                }
            }
        },
        value: osEvent__bindgen_ty_1 { v: 0 }, // not used
        def: osEvent__bindgen_ty_2 {
            message_id: core::ptr::null_mut(),
        }, // not used
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::boxed::Box;
    use blueos::{
        scheduler,
        thread::{Builder, Entry},
    };
    use blueos_test_macro::test;
    use core::sync::atomic::{AtomicU32, Ordering};

    // Test helper function to create a test thread
    fn create_test_thread(entry: Entry) -> *mut OsThread {
        let thread = Builder::new(entry).start();
        let os_thread = Box::new(OsThread::with_default_name(thread.clone()));
        os_thread.init_event_flags();
        let ptr = Box::into_raw(os_thread);
        // Store the OsThread in the thread's alien pointer
        thread
            .lock()
            .set_alien_ptr(unsafe { NonNull::new_unchecked(ptr as *mut core::ffi::c_void) });
        ptr
    }

    // Test helper function to clean up test thread
    #[inline(never)]
    fn cleanup_test_thread(thread_ptr: *mut OsThread) {
        if !thread_ptr.is_null() {
            unsafe {
                let _ = Box::from_raw(thread_ptr);
            }
        }
    }

    #[test]
    fn test_os_signal_set_basic() {
        // Test basic signal setting functionality
        let thread_ptr = create_test_thread(Entry::Closure(Box::new(|| {
            // do nothing
        })));

        // Set signal flags using osSignalSet
        let result = osSignalSet(thread_ptr as osThreadId, 0x01);
        assert_ne!(result, SIGNAL_ERROR);
        scheduler::yield_me(); // wait for test thread to finish
        cleanup_test_thread(thread_ptr);
    }

    #[test]
    fn test_os_signal_clear() {
        // Test basic signal clearing functionality
        let thread_ptr = create_test_thread(Entry::Closure(Box::new(|| {
            // do nothing
        })));
        // Set signal flags first using osSignalSet
        let set_result = osSignalSet(thread_ptr as osThreadId, 0x07);
        assert_ne!(set_result, SIGNAL_ERROR);
        // Clear some flags using osSignalClear
        let result = osSignalClear(thread_ptr as osThreadId, 0x03);
        assert_ne!(result, SIGNAL_ERROR);
        scheduler::yield_me(); // wait for test thread to finish
        cleanup_test_thread(thread_ptr);
    }

    #[test]
    fn test_os_signal_wait_any() {
        let thread_ptr = create_test_thread(Entry::Closure(Box::new(|| {
            let event = __osSignalWait(0, 100);
            assert_eq!(event.status, osStatus_osEventSignal);
        })));
        scheduler::yield_me();
        // Set a signal flag using osSignalSet
        let set_result = osSignalSet(thread_ptr as osThreadId, 0x02);
        assert_ne!(set_result, SIGNAL_ERROR);
        scheduler::suspend_me_for(10); // wait for test thread to finish
        cleanup_test_thread(thread_ptr);
    }

    #[test]
    fn test_os_signal_wait_all() {
        // Test waiting for all specified signal flags
        let thread_ptr = create_test_thread(Entry::Closure(Box::new(|| {
            let event = __osSignalWait(0x03, 100);
            assert_eq!(event.status, osStatus_osEventSignal);
        })));
        scheduler::yield_me();
        // Set multiple signal flags using osSignalSet
        let set_result = osSignalSet(thread_ptr as osThreadId, 0x03);
        assert_ne!(set_result, SIGNAL_ERROR);
        scheduler::suspend_me_for(10); // wait for test thread to finish
        cleanup_test_thread(thread_ptr);
    }
}
