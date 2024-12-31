use crate::blue_kernel::{
    error::code,
    sync::{ipc_common::*, message_queue::RtMessageQueue},
};
use core::{ffi, ptr::null_mut};

#[no_mangle]
pub unsafe extern "C" fn rt_mq_init(
    mq: *mut RtMessageQueue,
    name: *const ffi::c_char,
    msgpool: *mut ffi::c_void,
    msg_size: usize,
    pool_size: usize,
    flag: u8,
) -> i32 {
    assert!(!mq.is_null());
    assert!((flag == IPC_WAIT_MODE_FIFO as u8) || (flag == IPC_WAIT_MODE_PRIO as u8));
    #[allow(unused_mut, unused_assignments)]
    let mut queue_working_mode = IPC_SYS_QUEUE_FIFO as u8;
    #[cfg(feature = "messagequeue_priority")]
    {
        queue_working_mode = IPC_SYS_QUEUE_PRIO as u8;
    }
    (*mq).init(
        name,
        msgpool as *mut u8,
        msg_size as usize,
        pool_size as usize,
        queue_working_mode as u8,
        flag,
    )
}

#[no_mangle]
pub unsafe extern "C" fn rt_mq_detach(mq: *mut RtMessageQueue) -> i32 {
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
) -> *mut RtMessageQueue {
    #[allow(unused_mut, unused_assignments)]
    let mut queue_working_mode = IPC_SYS_QUEUE_FIFO as u8;
    #[cfg(feature = "messagequeue_priority")]
    {
        queue_working_mode = IPC_SYS_QUEUE_PRIO as u8;
    }
    RtMessageQueue::new_raw(
        name,
        msg_size as usize,
        max_msgs as usize,
        queue_working_mode,
        flag,
    )
}

#[cfg(feature = "heap")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_delete(mq: *mut RtMessageQueue) -> i32 {
    assert!(mq != null_mut());

    (*mq).delete_raw();

    code::EOK.to_errno()
}

#[no_mangle]
pub unsafe extern "C" fn rt_mq_entry(mq: *mut RtMessageQueue) -> u16 {
    assert!(!mq.is_null());
    (*mq).inner_queue.count() as u16
}

#[no_mangle]
pub unsafe extern "C" fn rt_mq_send(
    mq: *mut RtMessageQueue,
    buffer: *const ffi::c_void,
    size: usize,
) -> i32 {
    assert!(!mq.is_null());
    (*mq).send(buffer as *const u8, size as usize)
}

#[no_mangle]
pub unsafe extern "C" fn rt_mq_send_interruptible(
    mq: *mut RtMessageQueue,
    buffer: *const ffi::c_void,
    size: usize,
) -> i32 {
    assert!(!mq.is_null());
    (*mq).send_interruptible(buffer as *const u8, size as usize)
}

#[no_mangle]
pub unsafe extern "C" fn rt_mq_send_killable(
    mq: *mut RtMessageQueue,
    buffer: *const ffi::c_void,
    size: usize,
) -> i32 {
    assert!(!mq.is_null());
    (*mq).send_killable(buffer as *const u8, size as usize)
}

#[no_mangle]
pub unsafe extern "C" fn rt_mq_send_wait(
    mq: *mut RtMessageQueue,
    buffer: *const ffi::c_void,
    size: usize,
    timeout: i32,
) -> i32 {
    assert!(!mq.is_null());
    (*mq).send_wait(buffer as *const u8, size as usize, timeout)
}

#[no_mangle]
pub unsafe extern "C" fn rt_mq_send_wait_interruptible(
    mq: *mut RtMessageQueue,
    buffer: *const ffi::c_void,
    size: usize,
    timeout: i32,
) -> i32 {
    assert!(!mq.is_null());
    (*mq).send_wait_interruptible(buffer as *const u8, size as usize, timeout)
}

#[no_mangle]
pub unsafe extern "C" fn rt_mq_send_wait_killable(
    mq: *mut RtMessageQueue,
    buffer: *const ffi::c_void,
    size: usize,
    timeout: i32,
) -> i32 {
    assert!(!mq.is_null());
    (*mq).send_wait_killable(buffer as *const u8, size as usize, timeout)
}

#[no_mangle]
pub unsafe extern "C" fn rt_mq_urgent(
    mq: *mut RtMessageQueue,
    buffer: *const ffi::c_void,
    size: usize,
) -> i32 {
    assert!(!mq.is_null());
    (*mq).urgent(buffer as *const u8, size as usize)
}

#[no_mangle]
pub unsafe extern "C" fn rt_mq_recv(
    mq: *mut RtMessageQueue,
    buffer: *mut ffi::c_void,
    size: usize,
    timeout: i32,
) -> usize {
    assert!(!mq.is_null());
    (*mq).receive(buffer as *mut u8, size as usize, timeout) as usize
}

#[no_mangle]
pub unsafe extern "C" fn rt_mq_recv_interruptible(
    mq: *mut RtMessageQueue,
    buffer: *mut ffi::c_void,
    size: usize,
    timeout: i32,
) -> usize {
    assert!(!mq.is_null());
    (*mq).receive_interruptible(buffer as *mut u8, size as usize, timeout) as usize
}

#[no_mangle]
pub unsafe extern "C" fn rt_mq_recv_killable(
    mq: *mut RtMessageQueue,
    buffer: *mut ffi::c_void,
    size: usize,
    timeout: i32,
) -> usize {
    assert!(!mq.is_null());
    (*mq).receive_killable(buffer as *mut u8, size as usize, timeout) as usize
}

#[no_mangle]
pub unsafe extern "C" fn rt_mq_control(
    mq: *mut RtMessageQueue,
    cmd: ffi::c_int,
    _arg: *mut ffi::c_void,
) -> i32 {
    assert!(!mq.is_null());
    (*mq).control(cmd, _arg as *mut u8)
}

#[cfg(feature = "messagequeue_priority")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_send_wait_prio(
    mq: *mut RtMessageQueue,
    buffer: *const ffi::c_void,
    size: usize,
    prio: i32,
    timeout: i32,
    suspend_flag: ffi::c_int,
) -> i32 {
    assert!(!mq.is_null());
    (*mq).send_wait_prio(
        buffer as *const u8,
        size as usize,
        prio,
        timeout,
        suspend_flag as u32,
    )
}

#[cfg(feature = "messagequeue_priority")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_recv_prio(
    mq: *mut RtMessageQueue,
    buffer: *mut ffi::c_void,
    size: usize,
    prio: *mut i32,
    timeout: i32,
    suspend_flag: ffi::c_int,
) -> usize {
    assert!(!mq.is_null());
    (*mq).receive_prio(
        buffer as *mut u8,
        size as usize,
        prio,
        timeout,
        suspend_flag as u32,
    )
}
