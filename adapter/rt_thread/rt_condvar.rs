use crate::bluekernel::{
    error::code,
    sync::{condvar::CondVar, lock::mutex::Mutex, wait_list::WaitMode},
};

#[no_mangle]
pub unsafe extern "C" fn rt_condvar_init(
    condvar: *mut CondVar,
    name: *const core::ffi::c_char,
    flag: u8,
) -> i32 {
    assert!(!condvar.is_null());
    let Ok(wait_mode) = WaitMode::try_from(flag as u32) else {
        return code::EINVAL.to_errno();
    };
    (*condvar).init(name, wait_mode);
    code::EOK.to_errno()
}

#[no_mangle]
pub unsafe extern "C" fn rt_condvar_detach(condvar: *mut CondVar) -> i32 {
    assert!(!condvar.is_null());
    (*condvar).detach();
    code::EOK.to_errno()
}

#[no_mangle]
pub unsafe extern "C" fn rt_condvar_wait(condvar: *mut CondVar, mutex: *mut Mutex) -> i32 {
    assert!(!condvar.is_null());
    let mutex_ref = unsafe { &mut (*mutex) };
    (*condvar)
        .wait(mutex_ref)
        .map_or_else(|e| e.to_errno(), |_| code::EOK.to_errno())
}

#[no_mangle]
pub unsafe extern "C" fn rt_condvar_notify(condvar: *mut CondVar) -> i32 {
    assert!(!condvar.is_null());
    (*condvar)
        .notify()
        .map_or_else(|e| e.to_errno(), |_| code::EOK.to_errno())
}

#[no_mangle]
pub unsafe extern "C" fn rt_condvar_notify_all(condvar: *mut CondVar) -> i32 {
    assert!(!condvar.is_null());
    (*condvar)
        .notify_all()
        .map_or_else(|e| e.to_errno(), |_| code::EOK.to_errno())
}
