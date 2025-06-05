use crate::kernel::{
    error::code,
    sync::{ipc_common::IPC_CMD_RESET, semaphore::Semaphore, wait_list::WaitMode},
    thread::SuspendFlag,
};
use core::{ffi, ptr};

#[no_mangle]
pub unsafe extern "C" fn rt_sem_init(
    sem: *mut Semaphore,
    name: *const ffi::c_char,
    value: u32,
    flag: u8,
) -> i32 {
    assert!(!sem.is_null());
    let Ok(wait_mode) = WaitMode::try_from(flag as u32) else {
        return code::EINVAL.to_errno();
    };
    (*sem).init(name, value as u16, wait_mode);
    code::EOK.to_errno()
}

#[no_mangle]
pub unsafe extern "C" fn rt_sem_detach(sem: *mut Semaphore) -> i32 {
    assert!(!sem.is_null());
    (*sem).detach();
    code::EOK.to_errno()
}
#[cfg(heap)]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_create(
    name: *const ffi::c_char,
    value: u32,
    flag: u8,
) -> *mut Semaphore {
    let wait_mode = match WaitMode::try_from(flag as u32) {
        Ok(mode) => mode,
        Err(_) => return ptr::null_mut(),
    };
    Semaphore::new_raw(name, value as u16, wait_mode)
}

#[cfg(heap)]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_delete(sem: *mut Semaphore) -> i32 {
    assert!(!sem.is_null());
    (*sem).delete_raw();
    code::EOK.to_errno()
}

#[no_mangle]
pub unsafe extern "C" fn rt_sem_take(sem: *mut Semaphore, time: i32) -> i32 {
    assert!(!sem.is_null());
    (*sem)
        .take_wait(time)
        .map_or_else(|e| e.to_errno(), |_| code::EOK.to_errno())
}

#[no_mangle]
pub unsafe extern "C" fn rt_sem_take_interruptible(sem: *mut Semaphore, time: i32) -> i32 {
    assert!(!sem.is_null());
    (*sem)
        .take_internal(time, SuspendFlag::Interruptible)
        .map_or_else(|e| e.to_errno(), |_| code::EOK.to_errno())
}

#[no_mangle]
pub unsafe extern "C" fn rt_sem_take_killable(sem: *mut Semaphore, time: i32) -> i32 {
    assert!(!sem.is_null());
    (*sem)
        .take_internal(time, SuspendFlag::Killable)
        .map_or_else(|e| e.to_errno(), |_| code::EOK.to_errno())
}

#[no_mangle]
pub unsafe extern "C" fn rt_sem_trytake(sem: *mut Semaphore) -> i32 {
    assert!(!sem.is_null());
    (*sem)
        .try_take()
        .map_or_else(|e| e.to_errno(), |_| code::EOK.to_errno())
}

#[no_mangle]
pub unsafe extern "C" fn rt_sem_release(sem: *mut Semaphore) -> i32 {
    assert!(!sem.is_null());
    (*sem)
        .release()
        .map_or_else(|e| e.to_errno(), |_| code::EOK.to_errno())
}

#[no_mangle]
pub unsafe extern "C" fn rt_sem_control(
    sem: *mut Semaphore,
    cmd: i32,
    arg: *const ffi::c_void,
) -> i32 {
    assert!(!sem.is_null());
    if cmd == IPC_CMD_RESET as i32 {
        (*sem)
            .reset(arg as u32)
            .map_or_else(|e| e.to_errno(), |_| code::EOK.to_errno())
    } else {
        code::ERROR.to_errno()
    }
}
