use crate::blue_kernel::{error::code, sync::lock::rwlock::RtRwLock};

#[no_mangle]
pub unsafe extern "C" fn rt_rwlock_init(
    rwlock: *mut RtRwLock,
    name: *const core::ffi::c_char,
    flag: u8,
) -> i32 {
    assert!(!rwlock.is_null());
    (*rwlock).init(name, flag);
    code::EOK.to_errno()
}

#[no_mangle]
pub unsafe extern "C" fn rt_rwlock_detach(rwlock: *mut RtRwLock) -> i32 {
    assert!(!rwlock.is_null());
    (*rwlock).detach();
    code::EOK.to_errno()
}

#[no_mangle]
pub unsafe extern "C" fn rt_rwlock_lock_read(rwlock: *mut RtRwLock) -> i32 {
    assert!(!rwlock.is_null());
    (*rwlock).lock_read()
}

#[no_mangle]
pub unsafe extern "C" fn rt_rwlock_lock_write(rwlock: *mut RtRwLock) -> i32 {
    assert!(!rwlock.is_null());
    (*rwlock).lock_write()
}

#[no_mangle]
pub unsafe extern "C" fn rt_rwlock_try_lock_read(rwlock: *mut RtRwLock) -> i32 {
    assert!(!rwlock.is_null());
    (*rwlock).try_lock_read()
}

#[no_mangle]
pub unsafe extern "C" fn rt_rwlock_try_lock_write(rwlock: *mut RtRwLock) -> i32 {
    assert!(!rwlock.is_null());
    (*rwlock).try_lock_write()
}

#[no_mangle]
pub unsafe extern "C" fn rt_rwlock_unlock(rwlock: *mut RtRwLock) -> i32 {
    assert!(!rwlock.is_null());
    (*rwlock).unlock()
}
