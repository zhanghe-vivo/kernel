use crate::blue_kernel::{error::code, sync::{mailbox::Mailbox, ipc_common::IPC_CMD_RESET}};
use core::ffi;

#[no_mangle]
pub unsafe extern "C" fn rt_mb_init(
    mb: *mut Mailbox,
    name: *const ffi::c_char,
    msgpool: *mut ffi::c_void,
    size: usize,
    flag: u8,
) -> i32 {
    assert!(!mb.is_null());

    (*mb).init(name, msgpool as *mut u8, size as usize, flag);

    code::EOK.to_errno()
}

#[no_mangle]
pub unsafe extern "C" fn rt_mb_detach(mb: *mut Mailbox) -> i32 {
    assert!(!mb.is_null());

    (*mb).detach();

    return code::EOK.to_errno();
}

#[cfg(feature = "heap")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_create(
    name: *const ffi::c_char,
    size: usize,
    flag: u8,
) -> *mut Mailbox {
    Mailbox::new_raw(name, size as usize, flag)
}

#[cfg(feature = "heap")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_delete(mb: *mut Mailbox) -> i32 {
    assert!(!mb.is_null());

    (*mb).delete_raw();

    code::EOK.to_errno()
}

#[no_mangle]
pub unsafe extern "C" fn rt_mb_send_wait(mb: *mut Mailbox, value: usize, timeout: i32) -> i32 {
    assert!(!mb.is_null());
    (*mb).send_wait(value as usize, timeout)
}

#[no_mangle]
pub unsafe extern "C" fn rt_mb_send_wait_interruptible(
    mb: *mut Mailbox,
    value: usize,
    timeout: i32,
) -> i32 {
    assert!(!mb.is_null());
    (*mb).send_wait_interruptible(value as usize, timeout)
}

#[no_mangle]
pub unsafe extern "C" fn rt_mb_send_wait_killable(
    mb: *mut Mailbox,
    value: usize,
    timeout: i32,
) -> i32 {
    assert!(!mb.is_null());
    (*mb).send_wait_killable(value as usize, timeout)
}

#[no_mangle]
pub unsafe extern "C" fn rt_mb_send(mb: *mut Mailbox, value: ffi::c_ulong) -> i32 {
    assert!(!mb.is_null());
    (*mb).send(value as usize)
}

#[no_mangle]
pub unsafe extern "C" fn rt_mb_send_interruptible(mb: *mut Mailbox, value: ffi::c_ulong) -> i32 {
    assert!(!mb.is_null());
    (*mb).send_interruptible(value as usize)
}

#[no_mangle]
pub unsafe extern "C" fn rt_mb_send_killable(mb: *mut Mailbox, value: ffi::c_ulong) -> i32 {
    assert!(!mb.is_null());
    (*mb).send_killable(value as usize)
}

#[no_mangle]
pub unsafe extern "C" fn rt_mb_urgent(mb: *mut Mailbox, value: ffi::c_ulong) -> i32 {
    assert!(!mb.is_null());
    (*mb).urgent(value as usize)
}

#[no_mangle]
pub unsafe extern "C" fn rt_mb_recv(
    mb: *mut Mailbox,
    value: *mut ffi::c_ulong,
    timeout: i32,
) -> i32 {
    assert!(!mb.is_null());
    let mut receive_val = 0usize;
    let ret_val = (*mb).receive(&mut receive_val, timeout);
    *value = receive_val as ffi::c_ulong;
    ret_val
}

#[no_mangle]
pub unsafe extern "C" fn rt_mb_recv_interruptible(
    mb: *mut Mailbox,
    value: *mut ffi::c_ulong,
    timeout: i32,
) -> i32 {
    assert!(!mb.is_null());
    let mut receive_val = 0usize;
    let ret_val = (*mb).receive_interruptible(&mut receive_val, timeout);
    *value = receive_val as ffi::c_ulong;
    ret_val
}

#[no_mangle]
pub unsafe extern "C" fn rt_mb_recv_killable(
    mb: *mut Mailbox,
    value: *mut ffi::c_ulong,
    timeout: i32,
) -> i32 {
    assert!(!mb.is_null());
    let mut receive_val = 0usize;
    let ret_val = (*mb).receive_killable(&mut receive_val, timeout);
    *value = receive_val as ffi::c_ulong;
    ret_val
}

#[no_mangle]
pub unsafe extern "C" fn rt_mb_control(
    mb: *mut Mailbox,
    cmd: ffi::c_int,
    _arg: *mut ffi::c_void,
) -> i32 {
    assert!(!mb.is_null());
    if cmd == IPC_CMD_RESET as ffi::c_int {
        (*mb).reset()
    } else {
        code::ERROR.to_errno()
    }
}
