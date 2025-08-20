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

use crate::os_adapter;
use alloc::boxed::Box;
use blueos::{
    allocator,
    config::DEFAULT_STACK_SIZE,
    irq,
    scheduler::{self, yield_me},
    sync::{atomic_wait, atomic_wake, ConstBarrier, SpinLockGuard},
    thread::{self, Entry, GlobalQueueVisitor, Thread},
    time,
    types::{Arc, ThreadPriority, Uint},
};
use cmsis_os2::{
    osPriority_t, osPriority_t_osPriorityError, osPriority_t_osPriorityHigh,
    osPriority_t_osPriorityISR, osPriority_t_osPriorityIdle, osPriority_t_osPriorityLow,
    osPriority_t_osPriorityNormal, osStatus_t, osStatus_t_osErrorISR, osStatus_t_osErrorParameter,
    osStatus_t_osErrorResource, osStatus_t_osOK, osThreadAttr_t, osThreadFunc_t, osThreadId_t,
    osThreadJoinable, osThreadState_t, osThreadState_t_osThreadBlocked,
    osThreadState_t_osThreadError, osThreadState_t_osThreadInactive, osThreadState_t_osThreadReady,
    osThreadState_t_osThreadRunning, osThreadState_t_osThreadTerminated,
};
use core::{
    mem,
    ptr::{self, NonNull},
    sync::atomic::{AtomicI8, AtomicUsize, Ordering},
};
use delegate::delegate;

type Joint = ConstBarrier<2>;

os_adapter! {
    "th" => Os2Thread: blueos::thread::Thread {
        joint: Joint,
        suspension: AtomicUsize,
        detached: AtomicI8,
    }
}

impl Os2Thread {
    delegate! {
        to self.inner() {
            pub fn priority(&self) -> ThreadPriority;
            pub fn state(&self) -> Uint;
            pub fn lock(&self) -> SpinLockGuard<'_, Thread>;
            pub fn stack_size(&self) -> usize;
            pub fn stack_base(&self) -> usize;
            pub fn saved_stack_usage(&self) -> usize;
            pub fn kill(&self, signum:i32) -> bool;
        }
    }
}

#[inline(always)]
pub fn to_os_state(state: u8) -> osThreadState_t {
    match state as Uint {
        thread::CREATED => osThreadState_t_osThreadInactive,
        thread::READY => osThreadState_t_osThreadReady,
        thread::RUNNING => osThreadState_t_osThreadRunning,
        thread::SUSPENDED => osThreadState_t_osThreadBlocked,
        thread::RETIRED => osThreadState_t_osThreadTerminated,
        _ => osThreadState_t_osThreadError,
    }
}

#[allow(non_upper_case_globals)]
#[inline(always)]
pub fn to_thread_state(state: osThreadState_t) -> u8 {
    match state {
        osThreadState_t_osThreadInactive => thread::CREATED as u8,
        osThreadState_t_osThreadReady => thread::READY as u8,
        osThreadState_t_osThreadRunning => thread::RUNNING as u8,
        osThreadState_t_osThreadBlocked => thread::SUSPENDED as u8,
        osThreadState_t_osThreadTerminated => thread::RETIRED as u8,
        _ => unreachable!("Unknown thread state"), // Invalid state
    }
}

// 57 priority in CMSIS-RTOS2, blueos at least 8 levels.
#[inline(always)]
pub fn to_os_priority(prio: ThreadPriority) -> osPriority_t {
    // Map to 57 levels.
    (prio << 3) as osPriority_t
}

#[inline(always)]
pub fn to_thread_priority(prio: osPriority_t) -> ThreadPriority {
    // Map from 57 levels.
    (prio >> 3) as ThreadPriority
}

// See https://arm-software.github.io/CMSIS_6/main/RTOS2/group__CMSIS__RTOS__ThreadMgmt.html#ga8df03548e89fbc56402a5cd584a505da
#[no_mangle]
pub extern "C" fn osThreadGetId() -> osThreadId_t {
    if let Some(alien_ptr) = scheduler::current_thread().get_alien_ptr() {
        alien_ptr.as_ptr() as osThreadId_t
    } else {
        log::warn!("not a cmsis thread");
        ptr::null_mut()
    }
}

// See https://arm-software.github.io/CMSIS_6/main/RTOS2/group__CMSIS__RTOS__ThreadMgmt.html#ga48d68b8666d99d28fa646ee1d2182b8f.
#[no_mangle]
pub extern "C" fn osThreadNew(
    func: osThreadFunc_t,
    arg: *mut core::ffi::c_void,
    attr: *const osThreadAttr_t,
) -> osThreadId_t {
    if irq::is_in_irq() {
        return ptr::null_mut();
    }
    let Some(func) = func else {
        return ptr::null_mut();
    };
    let mut merge_attr = osThreadAttr_t {
        name: ptr::null(),
        attr_bits: 0,
        cb_size: 0,
        cb_mem: ptr::null_mut(),
        stack_mem: ptr::null_mut(),
        stack_size: DEFAULT_STACK_SIZE as u32,
        priority: osPriority_t_osPriorityNormal,
        tz_module: 0,
        reserved: 0,
    };
    if !attr.is_null() {
        merge_attr = unsafe { *attr };
        if !merge_attr.cb_mem.is_null() {
            if merge_attr.cb_size < mem::size_of::<Os2Thread>() as u32 {
                log::error!("osThreadNew: cb_size is too small");
                return ptr::null_mut();
            }
        } else if merge_attr.cb_size != 0 {
            log::error!("osThreadNew: cb_size must be 0 when cb_mem isn't provided");
            return ptr::null_mut();
        }
        // Check stack param.
        if !merge_attr.stack_mem.is_null()
            && (merge_attr.stack_mem as u32 & 0xef != 0
                || merge_attr.stack_size < 128
                || merge_attr.stack_size > 0x7fff_ffff)
        {
            log::error!("osThreadNew: stack_mem must be aligned to 128 bytes, not greater than 0x7fff_ffff, and stack_size must be at least 128 bytes");
            return ptr::null_mut();
        }
        if merge_attr.priority == osPriority_t_osPriorityError {
            merge_attr.priority = osPriority_t_osPriorityNormal;
        } else if merge_attr.priority < osPriority_t_osPriorityIdle
            || merge_attr.priority > osPriority_t_osPriorityISR
        {
            log::error!("osThreadNew: invalid priority");
            return ptr::null_mut();
        }
    }
    let mut stack = thread::Stack::Raw {
        base: merge_attr.stack_mem as usize,
        size: merge_attr.stack_size as usize,
    };
    // All checks passed, create stack first.
    if merge_attr.stack_mem.is_null() {
        let mut stack_size = merge_attr.stack_size;
        if stack_size == 0 {
            stack_size = DEFAULT_STACK_SIZE as u32;
        }
        let stack_memory = blueos::allocator::malloc_align(stack_size as usize, 16) as *mut u8;
        if stack_memory.is_null() {
            log::error!("osThreadNew: failed to allocate stack memory");
            return ptr::null_mut();
        }
        stack = thread::Stack::Raw {
            base: stack_memory as usize,
            size: stack_size as usize,
        };
    }
    let entry = Box::new(move || unsafe {
        func(arg);
        osThreadExit();
    });
    let mut t = thread::Builder::new(thread::Entry::Closure(entry))
        .set_priority(to_thread_priority(merge_attr.priority))
        .set_stack(stack)
        .build();
    {
        let mut l = t.lock();
        l.register_once_signal_handler(libc::SIGTERM, move || {
            let current = scheduler::current_thread();
            let Some(alien_ptr) = current.get_alien_ptr() else {
                return;
            };
            let t = unsafe { &mut *(alien_ptr.as_ptr() as *mut Os2Thread) };
            exit_os2_thread(t);
            scheduler::retire_me();
        });
        if merge_attr.stack_mem.is_null() {
            let stack_base = t.stack_base();
            l.set_cleanup(Entry::Closure(Box::new(move || {
                let stack_base = stack_base as *mut u8;
                blueos::allocator::free_align(stack_base, 16);
            })));
        }
    }

    let os_thread = Box::new(Os2Thread::with_name(t.clone(), merge_attr.name));
    let ptr: *mut Os2Thread = Box::into_raw(os_thread);
    // Store the Os2Thread in the thread's alien pointer.
    t.lock()
        .set_alien_ptr(unsafe { NonNull::new_unchecked(ptr as *mut core::ffi::c_void) });
    let ok =
        scheduler::queue_ready_thread(to_thread_state(osThreadState_t_osThreadInactive) as Uint, t);
    assert!(ok);
    ptr as osThreadId_t
}

// See https://arm-software.github.io/CMSIS_6/main/RTOS2/group__CMSIS__RTOS__ThreadMgmt.html#ga0aeaf349604f456e68e78f9d3b42e44b.
#[no_mangle]
pub extern "C" fn osThreadGetPriority(thread_id: osThreadId_t) -> osPriority_t {
    if irq::is_in_irq() {
        return osPriority_t_osPriorityError;
    }
    let t = unsafe { &*(thread_id as *const _ as *const Os2Thread) };

    to_os_priority(t.priority())
}

// See https://arm-software.github.io/CMSIS_6/main/RTOS2/group__CMSIS__RTOS__ThreadMgmt.html#ga861a420fb2d643115b06622903fb3bfb.
#[no_mangle]
pub extern "C" fn osThreadSetPriority(
    thread_id: osThreadId_t,
    priority: osPriority_t,
) -> osStatus_t {
    if irq::is_in_irq() {
        return osStatus_t_osErrorISR;
    }
    if thread_id.is_null() {
        return osStatus_t_osErrorParameter;
    }
    if !(osPriority_t_osPriorityIdle..=osPriority_t_osPriorityISR).contains(&priority) {
        return osStatus_t_osErrorParameter;
    }
    let t = unsafe { &mut *(thread_id as *const _ as *mut Os2Thread) };
    if to_os_state(t.state() as u8) == osThreadState_t_osThreadTerminated {
        // Cannot set priority for terminated thread.
        return osStatus_t_osErrorResource;
    }
    let mut guard = t.lock();
    guard.set_priority(to_thread_priority(priority));
    osStatus_t_osOK
}

// See https://arm-software.github.io/CMSIS_6/main/RTOS2/group__CMSIS__RTOS__ThreadMgmt.html#gad01c7ec26535b1de6b018bb9466720e2
#[no_mangle]
pub extern "C" fn osThreadYield() -> osStatus_t {
    if irq::is_in_irq() {
        return osStatus_t_osErrorISR;
    }
    yield_me();
    osStatus_t_osOK
}

// See https://arm-software.github.io/CMSIS_6/main/RTOS2/group__CMSIS__RTOS__ThreadMgmt.html#gac3230f3a55a297514b013ebf38f27e0a
#[no_mangle]
pub extern "C" fn osThreadGetName(thread_id: osThreadId_t) -> *const core::ffi::c_char {
    if irq::is_in_irq() || thread_id.is_null() {
        return ptr::null();
    }
    let t = unsafe { &*(thread_id as *const _ as *const Os2Thread) };
    t.name_bytes().as_ptr() as *const core::ffi::c_char
}

// See https://arm-software.github.io/CMSIS_6/main/RTOS2/group__CMSIS__RTOS__ThreadMgmt.html#gacc0a98b42f0a5928e12dc91dc76866b9
#[no_mangle]
pub extern "C" fn osThreadGetState(thread_id: osThreadId_t) -> osThreadState_t {
    if irq::is_in_irq() || thread_id.is_null() {
        return osThreadState_t_osThreadError;
    }
    let t = unsafe { &*(thread_id as *const _ as *const Os2Thread) };
    to_os_state(t.state() as u8)
}

#[no_mangle]
pub extern "C" fn osGetThreadStackSize(thread_id: osThreadId_t) -> usize {
    if irq::is_in_irq() || thread_id.is_null() {
        return 0;
    }

    let t = unsafe { &*(thread_id as *const _ as *const Os2Thread) };
    t.stack_size()
}

// See https://arm-software.github.io/CMSIS_6/main/RTOS2/group__CMSIS__RTOS__ThreadMgmt.html#ga9c83bd5dd8de329701775d6ef7012720
// Returns the remaining stack space for the specified thread.
// use previously saved stack usage for indication, not accurate in SMP
#[no_mangle]
pub extern "C" fn osGetThreadStackSpace(thread_id: osThreadId_t) -> usize {
    if irq::is_in_irq() || thread_id.is_null() {
        return 0;
    }
    let t = unsafe { &*(thread_id as *const _ as *const Os2Thread) };
    t.stack_size().wrapping_sub(t.saved_stack_usage())
}

fn exit_os2_thread(t: &mut Os2Thread) {
    let detached = t.detached.swap(-1, Ordering::SeqCst);
    if detached == 0 {
        t.joint.wait();
    } else {
        drop_os2_thread(t);
    }
}

// See https://arm-software.github.io/CMSIS_6/main/RTOS2/group__CMSIS__RTOS__ThreadMgmt.html#gaddaa452dd7610e4096647a566d3556fc
#[no_mangle]
pub extern "C" fn osThreadExit() {
    if irq::is_in_irq() {
        panic!("osThreadExit called in IRQ context");
    }
    let Some(alien_ptr) = scheduler::current_thread().get_alien_ptr() else {
        panic!("osThreadExit called in an invalid state");
    };
    let t = unsafe { &mut *(alien_ptr.as_ptr() as *mut Os2Thread) };
    exit_os2_thread(t)
}

// See https://arm-software.github.io/CMSIS_6/main/RTOS2/group__CMSIS__RTOS__ThreadMgmt.html#ga495b3f812224e7301f23a691793765db
#[no_mangle]
#[allow(non_upper_case_globals)]
pub extern "C" fn osThreadGetCount() -> usize {
    if irq::is_in_irq() {
        return 0;
    }
    let mut count = 0;
    let mut global_queue_visitor = GlobalQueueVisitor::new();
    while let Some(thread) = global_queue_visitor.next() {
        if let Some(alien_ptr) = thread.get_alien_ptr() {
            let t = unsafe { &*(alien_ptr.as_ptr() as *const Os2Thread) };
            count += match to_os_state(t.state() as u8) {
                osThreadState_t_osThreadBlocked
                | osThreadState_t_osThreadReady
                | osThreadState_t_osThreadRunning => 1,
                _ => 0,
            };
        }
    }
    count
}

// See https://arm-software.github.io/CMSIS_6/main/RTOS2/group__CMSIS__RTOS__ThreadMgmt.html#ga5606604d56e21ece1a654664be877439
#[no_mangle]
pub extern "C" fn osThreadEnumerate(thread_ids: *mut osThreadId_t, count: usize) -> u32 {
    if irq::is_in_irq() {
        return 0;
    }
    let mut global_queue_visitor = GlobalQueueVisitor::new();
    let mut index = 0;
    while let Some(thread) = global_queue_visitor.next() {
        if index < count {
            unsafe {
                let Some(alien_ptr) = thread.get_alien_ptr() else {
                    continue;
                };
                *thread_ids.add(index) = alien_ptr.as_ptr() as osThreadId_t;
                index += 1;
            }
        }
    }
    index as u32
}

#[no_mangle]
#[allow(non_upper_case_globals)]
pub extern "C" fn osThreadSuspend(thread_id: osThreadId_t) -> osStatus_t {
    if irq::is_in_irq() {
        return osStatus_t_osErrorISR;
    }
    if thread_id.is_null() {
        return osStatus_t_osErrorParameter;
    }
    let current = scheduler::current_thread();
    let Some(alien_ptr) = current.get_alien_ptr() else {
        return osStatus_t_osErrorParameter;
    };
    let t = unsafe { &mut *(thread_id as *const _ as *mut Os2Thread) };
    // If this thread is suspending its self.
    if ptr::eq(t, alien_ptr.as_ptr() as *mut Os2Thread) {
        atomic_wait(&t.suspension, 0, None);
        return osStatus_t_osOK;
    }
    // FIXME: We should use SIGUSR1 here, however it's not defined yet.
    if !t.kill(libc::SIGHUP) {
        return osStatus_t_osErrorResource;
    }
    let mut l = t.lock();
    let suspended = Arc::new(Joint::new());
    {
        // let suspension = t.suspension.clone();
        let suspended = suspended.clone();
        l.register_once_signal_handler(libc::SIGHUP, move || {
            suspended.wait();
            let Some(alien_ptr) = current.get_alien_ptr() else {
                return;
            };
            let t = alien_ptr.as_ptr() as *mut Os2Thread;
            let t = unsafe { &*t };
            atomic_wait(&t.suspension, 0, None);
            // Reset the flag for reusing in the next suspension.
            // This should be safe since nested signal handling is impossible at the moment.
            let _ = t
                .suspension
                .compare_exchange(1, 0, Ordering::Release, Ordering::Relaxed);
        });
    }
    suspended.wait();
    osStatus_t_osOK
}

#[no_mangle]
pub extern "C" fn osThreadResume(thread_id: osThreadId_t) -> osStatus_t {
    if irq::is_in_irq() {
        return osStatus_t_osErrorISR;
    }
    if thread_id.is_null() {
        return osStatus_t_osErrorParameter;
    }
    let Some(alien_ptr) = scheduler::current_thread().get_alien_ptr() else {
        return osStatus_t_osErrorParameter;
    };
    let t = unsafe { &mut *(thread_id as *const _ as *mut Os2Thread) };
    // It's impossible to resume current thread in the current thread.
    if ptr::eq(t, alien_ptr.as_ptr() as *mut Os2Thread) {
        return osStatus_t_osErrorResource;
    }
    t.suspension.store(1, Ordering::Release);
    let Ok(woken) = atomic_wake(&t.suspension, 1) else {
        return osStatus_t_osErrorResource;
    };
    if woken != 1 {
        return osStatus_t_osErrorResource;
    }
    osStatus_t_osOK
}

fn drop_os2_thread(ptr: *mut Os2Thread) {
    unsafe { drop(Box::from_raw(ptr)) }
}

#[no_mangle]
pub extern "C" fn osThreadJoin(thread_id: osThreadId_t) -> osStatus_t {
    if irq::is_in_irq() {
        return osStatus_t_osErrorISR;
    }
    if thread_id.is_null() {
        return osStatus_t_osErrorParameter;
    }
    let t = unsafe { &mut *(thread_id as *const _ as *mut Os2Thread) };
    t.joint.wait();
    drop_os2_thread(t);
    // So that the thread joined can be retired.
    scheduler::yield_me();
    osStatus_t_osOK
}

#[no_mangle]
pub extern "C" fn osThreadDetach(thread_id: osThreadId_t) -> osStatus_t {
    if irq::is_in_irq() {
        return osStatus_t_osErrorISR;
    }
    if thread_id.is_null() {
        return osStatus_t_osErrorParameter;
    }
    let t = unsafe { &mut *(thread_id as *const _ as *mut Os2Thread) };
    let old_val = t
        .detached
        .compare_exchange(0, 1, Ordering::SeqCst, Ordering::Relaxed);
    let Err(failed_val) = old_val else {
        return osStatus_t_osOK;
    };
    if failed_val == 1 {
        return osStatus_t_osOK;
    }
    osStatus_t_osErrorResource
}

#[no_mangle]
pub extern "C" fn osThreadTerminate(thread_id: osThreadId_t) -> osStatus_t {
    if irq::is_in_irq() {
        return osStatus_t_osErrorISR;
    }
    if thread_id.is_null() {
        return osStatus_t_osErrorParameter;
    }

    let t = unsafe { &mut *(thread_id as *const _ as *mut Os2Thread) };

    if let Some(alien_ptr) = scheduler::current_thread().get_alien_ptr() {
        // If this thread is terminating its self. It's supposed to be detached.
        if ptr::eq(t, alien_ptr.as_ptr() as *mut Os2Thread) {
            exit_os2_thread(t);
            scheduler::retire_me();
            return osStatus_t_osErrorResource;
        }
    };

    if !t.kill(libc::SIGTERM) {
        return osStatus_t_osErrorResource;
    }
    // No matter what happens, push the thread into ready queue.
    // FIXME: There is still a little time window between invoking t.state() and
    // t's state got updated. We might provide an enforced queuing API.
    scheduler::queue_ready_thread(thread::SUSPENDED, t.inner().clone());
    osStatus_t_osOK
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::os2::delay;
    use blueos_test_macro::test;
    // TODO: missing call from ISR, AVH support assigned ISR handler and trigger interrupt
    // in running function, if qemu support it, we can support full test with CMSIS-RTOS2_Validation
    // below are same as CMSIS-RTOS2_VALIDATION2, please see
    // https://github.com/ARM-software/CMSIS-RTOS2_Validation/blob/main/Source/RV2_Thread.c

    // helper function
    extern "C" fn Th_SelfTerminate(arg: *mut core::ffi::c_void) {
        let _ = arg;
        delay::osDelay(10);
        osThreadTerminate(osThreadGetId());
    }
    // helper function
    extern "C" fn Th_osThreadGetCount_1(arg: *mut core::ffi::c_void) {
        let _ = arg;
        scheduler::suspend_me_for(time::WAITING_FOREVER);
    }
    // helper function
    extern "C" fn Th_osThreadEnumerate_1(arg: *mut core::ffi::c_void) {
        let _ = arg;
    }
    // helper function
    extern "C" fn Th_ThreadMultiInstance(arg: *mut core::ffi::c_void) {
        let cnt = arg as *mut u8;
        unsafe { *cnt += 1 };
    }

    // helper function
    extern "C" fn Th_Run(arg: *mut core::ffi::c_void) {
        let _ = arg;
        osThreadYield();
    }

    // helper function
    extern "C" fn Th_osThreadGetId_1(arg: *mut core::ffi::c_void) {
        let id = arg as *mut osThreadId_t;
        unsafe {
            *id = osThreadGetId();
        }
        osThreadTerminate(osThreadGetId());
    }

    // Call osThreadNew to create a thread
    // Call osThreadNew with null thread function
    // Call osThreadNew from ISR
    #[test]
    fn TC_osThreadNew_1() {
        let thread_id = osThreadNew(Some(Th_Run), ptr::null_mut(), ptr::null());
        assert!(!thread_id.is_null(), "Thread ID should be greater than 0");
        osThreadJoin(thread_id);
        let thread_id2 = osThreadNew(None, ptr::null_mut(), ptr::null());
        assert!(thread_id2.is_null(), "Thread ID should be NULL");
    }

    // Call osThreadGetName to retrieve a name of an unnamed thread
    // Call osThreadGetName to retrieve a name of a thread with assigned name
    // Call osThreadGetName from ISR
    // Call osThreadGetName with null object
    #[test]
    fn TC_osThreadGetName_1() {
        let mut attr = osThreadAttr_t {
            name: ptr::null(),
            attr_bits: 0,
            cb_mem: ptr::null_mut(),
            cb_size: 0,
            stack_mem: ptr::null_mut(),
            stack_size: DEFAULT_STACK_SIZE as u32,
            priority: osPriority_t_osPriorityHigh,
            tz_module: 0,
            reserved: 0,
        };
        let thread_id = osThreadNew(Some(Th_SelfTerminate), ptr::null_mut(), &attr as *const _);
        let name = osThreadGetName(thread_id);
        // Check if the name is null for unnamed thread,
        // now adapter always convert to bytes array, maybe changed after clarification
        // assert!(name.is_null(), "Thread name should be null");
        assert!(!name.is_null(), "Thread name should not be null");
        attr.name = c"thread0".as_ptr() as *const core::ffi::c_char;
        osThreadJoin(thread_id);
        let thread_id2 = osThreadNew(Some(Th_SelfTerminate), ptr::null_mut(), &attr as *const _);
        let name2 = osThreadGetName(thread_id2);
        assert!(!name2.is_null(), "Thread name should not be null");
        osThreadJoin(thread_id2);
        // test call from ISR
    }

    // Call osThreadGetState to retrieve the state of a running thread
    // Call osThreadGetState to retrieve the state of a ready thread
    // Call osThreadGetState from ISR
    // Call osThreadGetState to retrieve the state of a terminated thread
    // Call osThreadGetState with null object
    #[test]
    fn TC_osThreadGetState_1() {
        let mut cnt = 0;
        let attr = osThreadAttr_t {
            name: ptr::null(),
            attr_bits: 0,
            cb_mem: ptr::null_mut(),
            cb_size: 0,
            stack_mem: ptr::null_mut(),
            stack_size: DEFAULT_STACK_SIZE as u32,
            priority: osPriority_t_osPriorityNormal,
            tz_module: 0,
            reserved: 0,
        };
        let thread_id = osThreadNew(
            Some(Th_SelfTerminate),
            &mut cnt as *mut _ as *mut core::ffi::c_void,
            &attr as *const _,
        );
        let state = osThreadGetState(thread_id);
        assert!(
            state == osThreadState_t_osThreadReady
                || state == osThreadState_t_osThreadRunning
                || state == osThreadState_t_osThreadBlocked,
            "New thread should be in ready, running or blocked state"
        );
        osThreadJoin(thread_id);
        // test call from ISR
    }

    #[test]
    fn TC_osThreadGetStackSize_1() {
        let attr = osThreadAttr_t {
            name: ptr::null(),
            attr_bits: 0,
            cb_mem: ptr::null_mut(),
            cb_size: 0,
            stack_mem: ptr::null_mut(),
            stack_size: DEFAULT_STACK_SIZE as u32,
            priority: osPriority_t_osPriorityLow,
            tz_module: 0,
            reserved: 0,
        };

        let thread_id = osThreadNew(Some(Th_SelfTerminate), ptr::null_mut(), &attr as *const _);
        let stack_size = osGetThreadStackSize(thread_id);
        assert_eq!(
            stack_size, DEFAULT_STACK_SIZE,
            "Stack size should be {} for the new thread",
            DEFAULT_STACK_SIZE
        );
        // osThreadTerminate(thread_id);
        assert_eq!(
            osGetThreadStackSize(ptr::null_mut()),
            0,
            "Stack size for null thread id should be 0"
        );
    }

    // Call osThreadGetStackSpace to retrieve the unused stack space of a running thread
    // Call osThreadGetStackSpace to retrieve the unused stack space of a ready thread
    // Call osThreadGetStackSpace from ISR
    // Call osThreadGetStackSpace with null object
    #[test]
    fn TC_osThreadGetStackSpace_1() {
        let attr = osThreadAttr_t {
            name: ptr::null(),
            attr_bits: 0,
            cb_mem: ptr::null_mut(),
            cb_size: 0,
            stack_mem: ptr::null_mut(),
            stack_size: DEFAULT_STACK_SIZE as u32,
            priority: osPriority_t_osPriorityHigh,
            tz_module: 0,
            reserved: 0,
        };

        let thread_id = osThreadNew(Some(Th_SelfTerminate), ptr::null_mut(), &attr as *const _);
        let stack_space = osGetThreadStackSpace(thread_id);
        assert!(
            stack_space > 0 && stack_space < DEFAULT_STACK_SIZE,
            "Stack space should be great then 0 and less then {} for the new thread",
            DEFAULT_STACK_SIZE
        );

        // osThreadTerminate(thread_id);
        assert_eq!(
            osGetThreadStackSpace(ptr::null_mut()),
            0,
            "Stack space for null thread id should be 0"
        );
        osThreadJoin(thread_id);
    }

    // Call osThreadGetCount to retrieve the number of active threads
    // Call osThreadGetCount from ISR
    #[test]
    fn TC_osThreadGetCount_1() {
        let count = osThreadGetCount();
        let mut attr = osThreadAttr_t {
            name: ptr::null(),
            attr_bits: 0,
            cb_mem: ptr::null_mut(),
            cb_size: 0,
            stack_mem: ptr::null_mut(),
            stack_size: 0,
            priority: osPriority_t_osPriorityHigh,
            tz_module: 0,
            reserved: 0,
        };
        let id_0 = osThreadNew(
            Some(Th_osThreadGetCount_1),
            ptr::null_mut(),
            &attr as *const _,
        );
        assert!(!id_0.is_null(), "Thread ID should be greater than 0");
        attr.priority = osPriority_t_osPriorityLow;
        let id_1 = osThreadNew(Some(Th_SelfTerminate), ptr::null_mut(), &attr as *const _);
        assert_eq!(
            osThreadGetCount(),
            count + 2,
            "Thread count should be increased by 2 after creating new threads"
        );
        osThreadJoin(id_1);
        osThreadTerminate(id_0);
        osThreadJoin(id_0);
    }

    #[test]
    fn TC_osThreadEnumerate_1() {
        let mut attr = osThreadAttr_t {
            name: ptr::null(),
            attr_bits: 0,
            cb_mem: ptr::null_mut(),
            cb_size: 0,
            stack_mem: ptr::null_mut(),
            stack_size: 0,
            priority: osPriority_t_osPriorityHigh,
            tz_module: 0,
            reserved: 0,
        };
        let id_0 = osThreadNew(
            Some(Th_osThreadEnumerate_1),
            ptr::null_mut(),
            &attr as *const _,
        );
        const N: usize = 16;
        assert!(!id_0.is_null(), "Thread ID should be greater than 0");
        attr.priority = osPriority_t_osPriorityLow;
        let id_1 = osThreadNew(Some(Th_SelfTerminate), ptr::null_mut(), &attr as *const _);
        let mut thread_ids: [osThreadId_t; N] = [ptr::null_mut(); N];
        let cnt = osThreadEnumerate(thread_ids.as_mut_ptr(), N) as usize;
        assert!(cnt > 0, "Thread enumeration count should be greater than 0");
        assert!(cnt < N, "Thread enumeration count should be less than N");
        let mut id_0_idx = usize::MAX;
        for (k, it) in thread_ids.iter().enumerate().take(N) {
            if *it == id_0 {
                id_0_idx = k;
                break;
            }
        }
        assert!(id_0_idx < N, "Thread ID should found in enumeration");
        let mut id_1_idx = usize::MAX;
        for (k, it) in thread_ids.iter().enumerate().take(N) {
            if *it == id_1 {
                id_1_idx = k;
                break;
            }
        }
        assert!(id_1_idx < N, "Thread ID should found in enumeration");
        assert_ne!(id_0_idx, id_1_idx);
        osThreadTerminate(id_0);
        osThreadJoin(id_0);
        osThreadTerminate(id_1);
        osThreadJoin(id_1);
    }

    #[test]
    fn TC_ThreadMultiInstance() {
        const N: usize = 16;
        let mut cnt = [0u8; N];
        let mut thread_ids = [ptr::null_mut(); N];
        let attr = osThreadAttr_t {
            name: ptr::null(),
            attr_bits: 0,
            cb_mem: ptr::null_mut(),
            cb_size: 0,
            stack_mem: ptr::null_mut(),
            stack_size: 0,
            priority: osPriority_t_osPriorityHigh,
            tz_module: 0,
            reserved: 0,
        };
        for i in 0..N {
            thread_ids[i] = osThreadNew(
                Some(Th_ThreadMultiInstance),
                &mut cnt[i] as *mut u8 as *mut core::ffi::c_void,
                &attr as *const _,
            );
            assert!(
                !thread_ids[i].is_null(),
                "Thread ID should be greater than 0"
            );
        }
        scheduler::suspend_me_for(128);
        for i in 0..N {
            osThreadJoin(thread_ids[i]);
            assert_eq!(
                cnt[i], 1,
                "Counter should be 1 before threads are suspended"
            );
        }
    }
}
