use crate::blue_kernel::{
    error::code,
    sync::{event, ipc_common::IPC_CMD_RESET, wait_list::WaitMode},
};
use core::{ffi, ptr};

#[no_mangle]
pub unsafe extern "C" fn rt_event_init(
    event: *mut event::Event,
    name: *const core::ffi::c_char,
    flag: u8,
) -> i32 {
    assert!(!event.is_null());
    let Ok(wait_mode) = WaitMode::try_from(flag as u32) else {
        return code::EINVAL.to_errno();
    };
    (*event).init(name, wait_mode);
    code::EOK.to_errno()
}

#[no_mangle]
pub unsafe extern "C" fn rt_event_detach(event: *mut event::Event) -> i32 {
    assert!(!event.is_null());
    (*event).detach();
    code::EOK.to_errno()
}

#[no_mangle]
pub unsafe extern "C" fn rt_event_create(
    name: *const core::ffi::c_char,
    flag: u8,
) -> *mut event::Event {
    let Ok(wait_mode) = WaitMode::try_from(flag as u32) else {
        return ptr::null_mut();
    };
    event::Event::new_raw(name, wait_mode)
}

#[no_mangle]
pub unsafe extern "C" fn rt_event_delete(event: *mut event::Event) -> i32 {
    assert!(!event.is_null());
    (*event).delete_raw();
    code::EOK.to_errno()
}

#[no_mangle]
pub unsafe extern "C" fn rt_event_send(event: *mut event::Event, set: u32) -> i32 {
    assert!(!event.is_null());
    (*event)
        .send(set)
        .map_or_else(|e| e.to_errno(), |_| code::EOK.to_errno())
}

#[no_mangle]
pub unsafe extern "C" fn rt_event_recv(
    event: *mut event::Event,
    set: u32,
    option: u8,
    timeout: i32,
    recved: *mut u32,
) -> i32 {
    assert!(!event.is_null());
    (*event).receive(set, option, timeout).map_or_else(
        |e| e.to_errno(),
        |value| {
            unsafe { *recved = value };
            code::EOK.to_errno()
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn rt_event_recv_interruptible(
    event: *mut event::Event,
    set: u32,
    option: u8,
    timeout: i32,
    recved: *mut u32,
) -> i32 {
    assert!(!event.is_null());
    (*event)
        .receive_interruptible(set, option, timeout)
        .map_or_else(
            |e| e.to_errno(),
            |value| {
                unsafe { *recved = value };
                code::EOK.to_errno()
            },
        )
}

#[no_mangle]
pub unsafe extern "C" fn rt_event_recv_killable(
    event: *mut event::Event,
    set: u32,
    option: u8,
    timeout: i32,
    recved: *mut u32,
) -> i32 {
    assert!(!event.is_null());
    (*event).receive_killable(set, option, timeout).map_or_else(
        |e| e.to_errno(),
        |value| {
            unsafe { *recved = value };
            code::EOK.to_errno()
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn rt_event_reset(event: *mut event::Event) -> i32 {
    assert!(!event.is_null());
    (*event)
        .reset()
        .map_or_else(|e| e.to_errno(), |_| code::EOK.to_errno())
}

#[no_mangle]
pub unsafe extern "C" fn rt_event_control(
    event: *mut event::Event,
    cmd: i32,
    _arg: *const ffi::c_void,
) -> i32 {
    assert!(!event.is_null());
    if cmd == IPC_CMD_RESET as i32 {
        (*event)
            .reset()
            .map_or_else(|e| e.to_errno(), |_| code::EOK.to_errno())
    } else {
        code::ERROR.to_errno()
    }
}
