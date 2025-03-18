use crate::kernel::{
    error::code,
    sync::{lock::rwlock::RwLock, wait_list::WaitMode},
};

#[no_mangle]
pub unsafe extern "C" fn rt_rwlock_init(
    rwlock: *mut RwLock,
    name: *const core::ffi::c_char,
    flag: u8,
) -> i32 {
    assert!(!rwlock.is_null());
    let Ok(wait_mode) = WaitMode::try_from(flag as u32) else {
        return code::EINVAL.to_errno();
    };
    (*rwlock).init(name, wait_mode);
    code::EOK.to_errno()
}

#[no_mangle]
pub unsafe extern "C" fn rt_rwlock_detach(rwlock: *mut RwLock) -> i32 {
    assert!(!rwlock.is_null());
    (*rwlock)
        .detach()
        .map_or_else(|e| e.to_errno(), |_| code::EOK.to_errno())
}

#[no_mangle]
pub unsafe extern "C" fn rt_rwlock_lock_read(rwlock: *mut RwLock) -> i32 {
    assert!(!rwlock.is_null());
    (*rwlock)
        .lock_read()
        .map_or_else(|e| e.to_errno(), |_| code::EOK.to_errno())
}

#[no_mangle]
pub unsafe extern "C" fn rt_rwlock_lock_write(rwlock: *mut RwLock) -> i32 {
    assert!(!rwlock.is_null());
    (*rwlock)
        .lock_write()
        .map_or_else(|e| e.to_errno(), |_| code::EOK.to_errno())
}

#[no_mangle]
pub unsafe extern "C" fn rt_rwlock_try_lock_read(rwlock: *mut RwLock) -> i32 {
    assert!(!rwlock.is_null());
    (*rwlock)
        .try_lock_read()
        .map_or_else(|e| e.to_errno(), |_| code::EOK.to_errno())
}

#[no_mangle]
pub unsafe extern "C" fn rt_rwlock_try_lock_write(rwlock: *mut RwLock) -> i32 {
    assert!(!rwlock.is_null());
    (*rwlock)
        .try_lock_write()
        .map_or_else(|e| e.to_errno(), |_| code::EOK.to_errno())
}

#[no_mangle]
pub unsafe extern "C" fn rt_rwlock_unlock(rwlock: *mut RwLock) -> i32 {
    assert!(!rwlock.is_null());
    (*rwlock)
        .unlock()
        .map_or_else(|e| e.to_errno(), |_| code::EOK.to_errno())
}
