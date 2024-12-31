use crate::blue_kernel::{error::code, sync::event};
use core::ffi;

#[no_mangle]
pub unsafe extern "C" fn rt_event_init(
    event: *mut event::RtEvent,
    name: *const core::ffi::c_char,
    flag: u8,
) -> i32 {
    assert!(!event.is_null());
    (*event).init(name, flag);
    code::EOK.to_errno()
}

#[no_mangle]
pub unsafe extern "C" fn rt_event_detach(event: *mut event::RtEvent) -> i32 {
    assert!(!event.is_null());
    (*event).detach();
    code::EOK.to_errno()
}

#[no_mangle]
pub unsafe extern "C" fn rt_event_create(
    name: *const core::ffi::c_char,
    flag: u8,
) -> *mut event::RtEvent {
    event::RtEvent::new_raw(name, flag)
}

#[no_mangle]
pub unsafe extern "C" fn rt_event_delete(event: *mut event::RtEvent) -> i32 {
    assert!(!event.is_null());
    (*event).delete_raw();
    code::EOK.to_errno()
}

#[no_mangle]
pub unsafe extern "C" fn rt_event_send(event: *mut event::RtEvent, set: u32) -> i32 {
    assert!(!event.is_null());
    (*event).send(set)
}

#[no_mangle]
pub unsafe extern "C" fn rt_event_recv(
    event: *mut event::RtEvent,
    set: u32,
    option: u8,
    timeout: i32,
    recved: *mut u32,
) -> i32 {
    assert!(!event.is_null());
    (*event).receive(set, option, timeout, recved as *mut u32)
}

#[no_mangle]
pub unsafe extern "C" fn rt_event_recv_interruptible(
    event: *mut event::RtEvent,
    set: u32,
    option: u8,
    timeout: i32,
    recved: *mut u32,
) -> i32 {
    assert!(!event.is_null());
    (*event).receive_interruptible(set, option, timeout, recved as *mut u32)
}

#[no_mangle]
pub unsafe extern "C" fn rt_event_recv_killable(
    event: *mut event::RtEvent,
    set: u32,
    option: u8,
    timeout: i32,
    recved: *mut i32,
) -> i32 {
    assert!(!event.is_null());
    (*event).receive_killable(set, option, timeout, recved as *mut u32)
}

#[no_mangle]
pub unsafe extern "C" fn rt_event_control(
    event: *mut event::RtEvent,
    cmd: i32,
    _arg: *const ffi::c_void,
) -> i32 {
    assert!(!event.is_null());
    (*event).control(cmd, _arg)
}
