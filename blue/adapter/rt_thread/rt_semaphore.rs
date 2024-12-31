use crate::blue_kernel::{
    error::code,
    sync::{ipc_common::IPC_CMD_RESET, semaphore::RtSemaphore},
    thread::SuspendFlag,
};

#[no_mangle]
pub unsafe extern "C" fn rt_sem_init(
    sem: *mut RtSemaphore,
    name: *const core::ffi::c_char,
    value: u32,
    flag: u8,
) -> i32 {
    assert!(!sem.is_null());
    (*sem).init(name, value as u16, flag);
    code::EOK.to_errno()
}

#[no_mangle]
pub unsafe extern "C" fn rt_sem_detach(sem: *mut RtSemaphore) -> i32 {
    assert!(!sem.is_null());
    (*sem).detach();
    code::EOK.to_errno()
}
#[cfg(feature = "heap")]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_create(
    name: *const core::ffi::c_char,
    value: u32,
    flag: u8,
) -> *mut RtSemaphore {
    RtSemaphore::new_raw(name, value as u16, flag)
}

#[cfg(feature = "heap")]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_delete(sem: *mut RtSemaphore) -> i32 {
    assert!(!sem.is_null());
    (*sem).delete_raw();
    code::EOK.to_errno()
}

#[no_mangle]
pub unsafe extern "C" fn rt_sem_take(sem: *mut RtSemaphore, time: i32) -> i32 {
    assert!(!sem.is_null());
    (*sem).take_wait(time)
}

#[no_mangle]
pub unsafe extern "C" fn rt_sem_take_interruptible(sem: *mut RtSemaphore, time: i32) -> i32 {
    assert!(!sem.is_null());
    (*sem).take_internal(time, SuspendFlag::Interruptible as u32)
}

#[no_mangle]
pub unsafe extern "C" fn rt_sem_take_killable(sem: *mut RtSemaphore, time: i32) -> i32 {
    assert!(!sem.is_null());
    (*sem).take_internal(time, SuspendFlag::Killable as u32)
}

#[no_mangle]
pub unsafe extern "C" fn rt_sem_trytake(sem: *mut RtSemaphore) -> i32 {
    assert!(!sem.is_null());
    (*sem).try_take()
}

#[no_mangle]
pub unsafe extern "C" fn rt_sem_release(sem: *mut RtSemaphore) -> i32 {
    assert!(!sem.is_null());
    (*sem).release()
}

#[no_mangle]
pub unsafe extern "C" fn rt_sem_control(
    sem: *mut RtSemaphore,
    cmd: i32,
    arg: *const core::ffi::c_void,
) -> i32 {
    assert!(!sem.is_null());
    if cmd == IPC_CMD_RESET as i32 {
        (*sem).reset(arg as u32);
        return code::EOK.to_errno();
    }
    code::ERROR.to_errno()
}
