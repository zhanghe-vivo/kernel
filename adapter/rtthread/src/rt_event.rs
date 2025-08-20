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

use crate::{common_objects::OsEventFlags, rt_def::*};
use blueos::{
    sync::event_flags::{EventFlags, EventFlagsMode},
    time,
    types::{Arc, ArcInner},
};
use core::{
    ffi::{c_char, c_void},
    mem::ManuallyDrop,
    ptr,
};

extern "C" {
    fn rt_object_init(obj: *mut rt_object, type_: rt_uint8_t, name: *const c_char) -> rt_err_t;
    fn rt_object_detach(obj: *mut rt_object) -> rt_err_t;
}

#[allow(non_camel_case_types)]
#[repr(transparent)]
pub struct rt_event(ArcInner<OsEventFlags>);

// rt_err_t rt_event_init(rt_event_t event, const char *name, rt_uint8_t flag)
#[no_mangle]
pub extern "C" fn rt_event_init(
    event: *mut rt_event,
    name: *const c_char,
    flag: rt_uint8_t,
) -> rt_err_t {
    if event.is_null() {
        return RT_EINVAL as rt_err_t;
    }
    let event_flags = Arc::new(EventFlags::const_new());
    unsafe {
        ptr::write(
            event as *mut ArcInner<OsEventFlags>,
            ArcInner::const_new(OsEventFlags::new(event_flags)),
        );
        rt_object_init(
            event as *mut rt_object,
            rt_object_class_type_RT_Object_Class_Event,
            name,
        );
    }
    let os_event =
        unsafe { ManuallyDrop::new(Arc::from_raw(event as *mut _ as *mut OsEventFlags)) };
    os_event.init(flag as u32);

    RT_EOK as rt_err_t
}

// rt_err_t rt_event_detach(rt_event_t event)
#[no_mangle]
pub extern "C" fn rt_event_detach(event: *mut rt_event) -> rt_err_t {
    if event.is_null() {
        return RT_EINVAL as rt_err_t;
    }
    let os_event =
        unsafe { ManuallyDrop::new(Arc::from_raw(event as *mut _ as *mut OsEventFlags)) };
    os_event.reset();

    unsafe { rt_object_detach(event as *mut rt_object) };

    RT_EOK as rt_err_t
}

// rt_event_t rt_event_create(const char *name, rt_uint8_t flag)
#[no_mangle]
pub extern "C" fn rt_event_create(name: *const c_char, flag: rt_uint8_t) -> *mut rt_event {
    let event_flags = Arc::new(EventFlags::const_new());
    let os_event = Arc::new(OsEventFlags::new(event_flags));
    os_event.init(flag as u32);

    let event = Arc::into_raw(os_event) as *mut rt_event;
    unsafe {
        rt_object_init(
            event as *mut rt_object,
            rt_object_class_type_RT_Object_Class_Event,
            name,
        )
    };
    event
}

// rt_err_t rt_event_delete(rt_event_t event)
#[no_mangle]
pub extern "C" fn rt_event_delete(event: *mut rt_event) -> rt_err_t {
    match rt_event_detach(event) {
        const { RT_EOK as rt_err_t } => {
            let _ = unsafe { Arc::from_raw(event as *mut _ as *mut OsEventFlags) };
            RT_EOK as rt_err_t
        }
        err => err,
    }
}

// rt_err_t rt_event_send(rt_event_t event, rt_uint32_t set)
#[no_mangle]
pub extern "C" fn rt_event_send(event: *mut rt_event, set: rt_uint32_t) -> rt_err_t {
    if event.is_null() {
        return RT_EINVAL as rt_err_t;
    }
    let os_event =
        unsafe { ManuallyDrop::new(Arc::from_raw(event as *mut _ as *mut OsEventFlags)) };
    match os_event.set(set as u32) {
        Ok(_) => RT_EOK as rt_err_t,
        Err(err) => RtErr::from(err).as_rt_err(),
    }
}

// rt_err_t rt_event_recv(rt_event_t event, rt_uint32_t set, rt_uint8_t option, rt_int32_t timeout, rt_uint32_t *recved)
#[no_mangle]
pub extern "C" fn rt_event_recv(
    event: *mut rt_event,
    set: rt_uint32_t,
    option: rt_uint8_t,
    timeout: rt_int32_t,
    recved: *mut rt_uint32_t,
) -> rt_err_t {
    if event.is_null() {
        return RT_EINVAL as rt_err_t;
    }
    let os_event =
        unsafe { ManuallyDrop::new(Arc::from_raw(event as *mut _ as *mut OsEventFlags)) };
    let mut mode = if option & RT_EVENT_FLAG_OR as u8 != 0 {
        EventFlagsMode::ANY
    } else {
        EventFlagsMode::ALL
    };
    if option & RT_EVENT_FLAG_CLEAR as u8 == 0 {
        mode |= EventFlagsMode::NO_CLEAR;
    }
    let timeout = if timeout == -1 {
        time::WAITING_FOREVER
    } else {
        timeout as usize
    };
    match os_event.wait(set as u32, mode, timeout) {
        Ok(flags) => {
            unsafe { *recved = flags as rt_uint32_t };
            RT_EOK as rt_err_t
        }
        Err(err) => RtErr::from(err).as_rt_err(),
    }
}

// rt_err_t rt_event_control(rt_event_t event, int cmd, void *arg)
#[no_mangle]
pub extern "C" fn rt_event_control(
    event: *mut rt_event,
    cmd: rt_uint8_t,
    arg: *mut c_void,
) -> rt_err_t {
    if event.is_null() {
        return RT_EINVAL as rt_err_t;
    }
    let os_event =
        unsafe { ManuallyDrop::new(Arc::from_raw(event as *mut _ as *mut OsEventFlags)) };
    match cmd as u32 {
        RT_IPC_CMD_RESET => {
            os_event.reset();
            RT_EOK as rt_err_t
        }
        _ => RT_ENOSYS as rt_err_t,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use blueos_test_macro::test;
    extern crate alloc;
    use alloc::ffi::CString;
    use core::ptr;

    #[test]
    fn test_rt_event_init() {
        // Test successful initialization
        let mut raw_event = unsafe { core::mem::MaybeUninit::<rt_event>::uninit() };
        let name = CString::new("evt").unwrap();
        let result = rt_event_init(raw_event.as_mut_ptr(), name.as_ptr(), 1);
        assert_eq!(result, RT_EOK as rt_err_t);

        let mut event = unsafe { raw_event.assume_init() };
        let os_event = unsafe { Arc::from_raw(&event.0 as *const _ as *mut OsEventFlags) };
        assert_eq!(os_event.name(), "evt");
        let result = rt_event_detach(Arc::into_raw(os_event) as *mut rt_event);
        assert_eq!(result, RT_EOK as rt_err_t);
        // memory will drop by event.
    }

    #[test]
    fn test_rt_event_create() {
        // Test successful creation
        let name = CString::new("evt").unwrap();
        let event = rt_event_create(name.as_ptr(), 2);

        assert!(!event.is_null());

        // Clean up
        let _ = rt_event_delete(event);
    }

    #[test]
    fn test_rt_event_send() {
        // Test successful event sending
        let name = CString::new("evt").unwrap();
        let event = rt_event_create(name.as_ptr(), 0);
        let result = rt_event_send(event, 0x01);

        assert_eq!(result, RT_EOK as rt_err_t);

        // Clean up
        let _ = rt_event_delete(event);
    }

    #[test]
    fn test_rt_event_recv() {
        // Test successful event receiving
        let name = CString::new("evt").unwrap();
        let event = rt_event_create(name.as_ptr(), 0);

        // Send an event first
        rt_event_send(event, 0x01);

        // Receive the event
        let mut recved: rt_uint32_t = 0;
        let result = rt_event_recv(event, 0x01, RT_EVENT_FLAG_OR as u8, 100, &mut recved);

        assert_eq!(result, RT_EOK as rt_err_t);
        assert_eq!(recved, 0x01);

        // Clean up
        let _ = rt_event_delete(event);
    }

    #[test]
    fn test_rt_event_recv_with_timeout() {
        // Test receiving with timeout
        let name = CString::new("evt").unwrap();
        let event = rt_event_create(name.as_ptr(), 0);
        let mut recved: rt_uint32_t = 0;

        // Try to receive with a short timeout (should timeout)
        let result = rt_event_recv(event, 0x01, RT_EVENT_FLAG_OR as u8, 1, &mut recved);

        // Should timeout (ETIMEOUT or similar)
        assert_ne!(result, RT_EOK as rt_err_t);

        // Clean up
        let _ = rt_event_delete(event);
    }

    #[test]
    fn test_rt_event_recv_all_flags() {
        // Test receiving with ALL flag mode
        let name = CString::new("evt").unwrap();
        let event = rt_event_create(name.as_ptr(), 0);

        // Send multiple flags
        rt_event_send(event, 0x03);

        // Receive with ALL mode
        let mut recved: rt_uint32_t = 0;
        let result = rt_event_recv(event, 0x03, 0, 100, &mut recved);

        assert_eq!(result, RT_EOK as rt_err_t);
        assert_eq!(recved, 0x03);

        // Clean up
        let _ = rt_event_delete(event);
    }

    #[test]
    fn test_rt_event_recv_no_clear() {
        // Test receiving with NO_CLEAR flag
        let name = CString::new("evt").unwrap();
        let event = rt_event_create(name.as_ptr(), 0);

        // Send an event
        rt_event_send(event, 0x01);

        // Receive without clearing
        let mut recved: rt_uint32_t = 0;
        let result = rt_event_recv(event, 0x01, RT_EVENT_FLAG_OR as u8, 100, &mut recved);

        assert_eq!(result, RT_EOK as rt_err_t);
        assert_eq!(recved, 0x01);

        // Try to receive again (should succeed because flags weren't cleared)
        let result2 = rt_event_recv(event, 0x01, RT_EVENT_FLAG_OR as u8, 100, &mut recved);
        assert_eq!(result2, RT_EOK as rt_err_t);

        // Clean up
        let _ = rt_event_delete(event);
    }

    #[test]
    fn test_rt_event_create_with_flag() {
        // Test creation with different flags
        let name = CString::new("evt").unwrap();
        let event = rt_event_create(name.as_ptr(), 0x0F);

        assert!(!event.is_null());

        // Clean up
        let _ = rt_event_delete(event);
    }

    #[test]
    fn test_rt_event_delete() {
        // Test successful deletion
        let name = CString::new("test_event").unwrap();
        let event = rt_event_create(name.as_ptr(), 0);
        let result = rt_event_delete(event);
        assert_eq!(result, RT_EOK as rt_err_t);
    }

    #[test]
    fn test_rt_event_send_null_pointer() {
        // Test sending with null pointer
        let result = rt_event_send(ptr::null_mut(), 0x01);
        assert_eq!(result, RT_EINVAL as rt_err_t);
    }

    #[test]
    fn test_rt_event_control_reset() {
        // Test event control with reset command
        let name = CString::new("evt").unwrap();
        let event = rt_event_create(name.as_ptr(), 0);

        // Send some flags first
        rt_event_send(event, 0x05);

        // Reset the event
        let result = rt_event_control(event, RT_IPC_CMD_RESET as rt_uint8_t, ptr::null_mut());
        assert_eq!(result, RT_EOK as rt_err_t);

        // Clean up
        let _ = rt_event_delete(event);
    }

    #[test]
    fn test_rt_event_control_null_pointer() {
        // Test control with null pointer
        let result = rt_event_control(
            ptr::null_mut(),
            RT_IPC_CMD_RESET as rt_uint8_t,
            ptr::null_mut(),
        );
        assert_eq!(result, RT_EINVAL as rt_err_t);
    }

    #[test]
    fn test_rt_event_integration() {
        // Integration test: create, send, receive, and delete
        let name = CString::new("evt").unwrap();
        let event = rt_event_create(name.as_ptr(), 0);
        assert!(!event.is_null());

        // Send multiple events
        let send_result = rt_event_send(event, 0x05);
        assert_eq!(send_result, RT_EOK as rt_err_t);

        // Receive events
        let mut recved: rt_uint32_t = 0;
        let recv_result = rt_event_recv(event, 0x05, RT_EVENT_FLAG_OR as u8, 100, &mut recved);
        assert_eq!(recv_result, RT_EOK as rt_err_t);
        assert_eq!(recved, 0x05);

        // Clean up
        let _ = rt_event_delete(event);
    }
}
