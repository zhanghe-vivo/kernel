use crate::blue_kernel::{
    error::code,
    sync::{condvar::CondVar, lock::mutex::Mutex},
};

#[no_mangle]
pub unsafe extern "C" fn rt_condvar_init(
    condvar: *mut CondVar,
    name: *const core::ffi::c_char,
    flag: u8,
) -> i32 {
    assert!(!condvar.is_null());
    (*condvar).init(name, flag);
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
    (*condvar).wait(mutex_ref)
}

#[no_mangle]
pub unsafe extern "C" fn rt_condvar_notify(condvar: *mut CondVar) -> i32 {
    assert!(!condvar.is_null());
    (*condvar).notify()
}

#[no_mangle]
pub unsafe extern "C" fn rt_condvar_notify_all(condvar: *mut CondVar) -> i32 {
    assert!(!condvar.is_null());
    (*condvar).notify_all()
}
