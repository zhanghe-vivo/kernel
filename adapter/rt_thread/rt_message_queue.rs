use crate::kernel::{
    error::code,
    sync::{ipc_common::*, message_queue::MessageQueue, wait_list::WaitMode},
};
use core::{ffi, ptr::null_mut};

#[no_mangle]
pub unsafe extern "C" fn rt_mq_init(
    mq: *mut MessageQueue,
    name: *const ffi::c_char,
    msgpool: *mut ffi::c_void,
    msg_size: usize,
    pool_size: usize,
    flag: u8,
) -> i32 {
    assert!(!mq.is_null());
    let Ok(wait_mode) = WaitMode::try_from(flag as u32) else {
        return code::EINVAL.to_errno();
    };

    #[allow(unused_mut, unused_assignments)]
    let mut queue_working_mode = IPC_SYS_QUEUE_FIFO as u8;
    #[cfg(feature = "messagequeue_priority")]
    {
        queue_working_mode = IPC_SYS_QUEUE_PRIO as u8;
    }
    (*mq)
        .init(
            name,
            msgpool as *mut u8,
            msg_size as usize,
            pool_size as usize,
            queue_working_mode as u8,
            wait_mode,
        )
        .to_errno()
}

#[no_mangle]
pub unsafe extern "C" fn rt_mq_detach(mq: *mut MessageQueue) -> i32 {
    assert!(!mq.is_null());
    (*mq).detach();
    code::EOK.to_errno()
}

#[cfg(feature = "heap")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_create(
    name: *const ffi::c_char,
    msg_size: usize,
    max_msgs: usize,
    flag: u8,
) -> *mut MessageQueue {
    let wait_mode = match WaitMode::try_from(flag as u32) {
        Ok(mode) => mode,
        Err(_) => return null_mut(),
    };

    #[allow(unused_mut, unused_assignments)]
    let mut queue_working_mode = IPC_SYS_QUEUE_FIFO as u8;
    #[cfg(feature = "messagequeue_priority")]
    {
        queue_working_mode = IPC_SYS_QUEUE_PRIO as u8;
    }
    MessageQueue::new_raw(
        name,
        msg_size as usize,
        max_msgs as usize,
        queue_working_mode,
        wait_mode,
    )
}

#[cfg(feature = "heap")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_delete(mq: *mut MessageQueue) -> i32 {
    assert!(mq != null_mut());

    (*mq).delete_raw();

    code::EOK.to_errno()
}

#[no_mangle]
pub unsafe extern "C" fn rt_mq_entry(mq: *mut MessageQueue) -> u16 {
    assert!(!mq.is_null());
    (*mq).inner_queue.count() as u16
}

#[no_mangle]
pub unsafe extern "C" fn rt_mq_send(
    mq: *mut MessageQueue,
    buffer: *const ffi::c_void,
    size: usize,
) -> i32 {
    assert!(!mq.is_null());
    match (*mq).send(buffer as *const u8, size) {
        Ok(_) => code::EOK.to_errno(),
        Err(e) => e.to_errno(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rt_mq_send_interruptible(
    mq: *mut MessageQueue,
    buffer: *const ffi::c_void,
    size: usize,
) -> i32 {
    assert!(!mq.is_null());
    match (*mq).send_interruptible(buffer as *const u8, size) {
        Ok(_) => code::EOK.to_errno(),
        Err(e) => e.to_errno(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rt_mq_send_killable(
    mq: *mut MessageQueue,
    buffer: *const ffi::c_void,
    size: usize,
) -> i32 {
    assert!(!mq.is_null());
    match (*mq).send_killable(buffer as *const u8, size) {
        Ok(_) => code::EOK.to_errno(),
        Err(e) => e.to_errno(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rt_mq_send_wait(
    mq: *mut MessageQueue,
    buffer: *const ffi::c_void,
    size: usize,
    timeout: i32,
) -> i32 {
    assert!(!mq.is_null());
    match (*mq).send_wait(buffer as *const u8, size, timeout) {
        Ok(_) => code::EOK.to_errno(),
        Err(e) => e.to_errno(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rt_mq_send_wait_interruptible(
    mq: *mut MessageQueue,
    buffer: *const ffi::c_void,
    size: usize,
    timeout: i32,
) -> i32 {
    assert!(!mq.is_null());
    match (*mq).send_wait_interruptible(buffer as *const u8, size, timeout) {
        Ok(_) => code::EOK.to_errno(),
        Err(e) => e.to_errno(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rt_mq_send_wait_killable(
    mq: *mut MessageQueue,
    buffer: *const ffi::c_void,
    size: usize,
    timeout: i32,
) -> i32 {
    assert!(!mq.is_null());
    match (*mq).send_wait_killable(buffer as *const u8, size, timeout) {
        Ok(_) => code::EOK.to_errno(),
        Err(e) => e.to_errno(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn rt_mq_urgent(
    mq: *mut MessageQueue,
    buffer: *const ffi::c_void,
    size: usize,
) -> i32 {
    assert!(!mq.is_null());
    (*mq).urgent(buffer as *const u8, size as usize)
}

#[no_mangle]
pub unsafe extern "C" fn rt_mq_recv(
    mq: *mut MessageQueue,
    buffer: *mut ffi::c_void,
    size: usize,
    timeout: i32,
) -> isize {
    assert!(!mq.is_null());
    match (*mq).receive(buffer as *mut u8, size, timeout) {
        Ok(received_size) => received_size as isize,
        Err(e) => e.to_errno() as isize,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rt_mq_recv_interruptible(
    mq: *mut MessageQueue,
    buffer: *mut ffi::c_void,
    size: usize,
    timeout: i32,
) -> isize {
    assert!(!mq.is_null());
    match (*mq).receive_interruptible(buffer as *mut u8, size, timeout) {
        Ok(received_size) => received_size as isize,
        Err(e) => e.to_errno() as isize,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rt_mq_recv_killable(
    mq: *mut MessageQueue,
    buffer: *mut ffi::c_void,
    size: usize,
    timeout: i32,
) -> isize {
    assert!(!mq.is_null());
    match (*mq).receive_killable(buffer as *mut u8, size, timeout) {
        Ok(received_size) => received_size as isize,
        Err(e) => e.to_errno() as isize,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rt_mq_control(
    mq: *mut MessageQueue,
    cmd: ffi::c_int,
    _arg: *mut ffi::c_void,
) -> i32 {
    assert!(!mq.is_null());
    if cmd == IPC_CMD_RESET as ffi::c_int {
        (*mq)
            .reset()
            .map_or_else(|e| e.to_errno(), |_| code::EOK.to_errno())
    } else {
        code::ERROR.to_errno()
    }
}

#[cfg(feature = "messagequeue_priority")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_send_wait_prio(
    mq: *mut MessageQueue,
    buffer: *const ffi::c_void,
    size: usize,
    prio: i32,
    timeout: i32,
    suspend_flag: ffi::c_int,
) -> i32 {
    assert!(!mq.is_null());
    (*mq)
        .send_wait_prio(
            buffer as *const u8,
            size as usize,
            prio,
            timeout,
            suspend_flag as u32,
        )
        .map_or_else(|e| e.to_errno(), |_| code::EOK.to_errno())
}

#[cfg(feature = "messagequeue_priority")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_recv_prio(
    mq: *mut MessageQueue,
    buffer: *mut ffi::c_void,
    size: usize,
    prio: *mut i32,
    timeout: i32,
    suspend_flag: ffi::c_int,
) -> isize {
    assert!(!mq.is_null());
    match (*mq).receive_prio(
        buffer as *mut u8,
        size as usize,
        prio,
        timeout,
        suspend_flag as u32,
    ) {
        Ok(received_size) => received_size as isize,
        Err(e) => e.to_errno() as isize,
    }
}
