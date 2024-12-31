use crate::blue_kernel::{
    error::code,
    sync::{condvar::RtCondVar, lock::mutex::RtMutex},
};

#[no_mangle]
pub unsafe extern "C" fn rt_condvar_init(
    condvar: *mut RtCondVar,
    name: *const core::ffi::c_char,
    flag: u8,
) -> i32 {
    assert!(!condvar.is_null());
    (*condvar).init(name, flag);
    code::EOK.to_errno()
}

#[no_mangle]
pub unsafe extern "C" fn rt_condvar_detach(condvar: *mut RtCondVar) -> i32 {
    assert!(!condvar.is_null());
    (*condvar).detach();
    code::EOK.to_errno()
}

#[no_mangle]
pub unsafe extern "C" fn rt_condvar_wait(condvar: *mut RtCondVar, mutex: *mut RtMutex) -> i32 {
    assert!(!condvar.is_null());
    let mutex_ref = unsafe { &mut (*mutex) };
    (*condvar).wait(mutex_ref)
}

#[no_mangle]
pub unsafe extern "C" fn rt_condvar_notify(condvar: *mut RtCondVar) -> i32 {
    assert!(!condvar.is_null());
    (*condvar).notify()
}

#[no_mangle]
pub unsafe extern "C" fn rt_condvar_notify_all(condvar: *mut RtCondVar) -> i32 {
    assert!(!condvar.is_null());
    (*condvar).notify_all()
}
