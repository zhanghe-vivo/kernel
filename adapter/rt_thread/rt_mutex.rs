use crate::bluekernel::{error::code, sync::lock::mutex::Mutex, thread::SuspendFlag};
use core::ffi;

#[no_mangle]
pub unsafe extern "C" fn rt_mutex_init(
    mutex: *mut Mutex,
    name: *const ffi::c_char,
    _flag: ffi::c_uchar,
) -> i32 {
    assert!(!mutex.is_null());
    (*mutex).init(name);
    code::EOK.to_errno()
}

#[no_mangle]
pub unsafe extern "C" fn rt_mutex_detach(mutex: *mut Mutex) -> i32 {
    assert!(!mutex.is_null());
    (*mutex).detach();
    code::EOK.to_errno()
}

#[cfg(feature = "heap")]
#[no_mangle]
pub unsafe extern "C" fn rt_mutex_create(
    name: *const ffi::c_char,
    _flag: ffi::c_uchar,
) -> *mut Mutex {
    Mutex::new_raw(name)
}

#[cfg(feature = "heap")]
#[no_mangle]
pub unsafe extern "C" fn rt_mutex_delete(mutex: *mut Mutex) -> i32 {
    assert!(!mutex.is_null());
    (*mutex).delete_raw();
    code::EOK.to_errno()
}

#[no_mangle]
pub unsafe extern "C" fn rt_mutex_take(mutex: *mut Mutex, time: i32) -> i32 {
    assert!(!mutex.is_null());
    (*mutex)
        .lock_wait(time)
        .map_or_else(|e| e.to_errno(), |_| code::EOK.to_errno())
}

#[no_mangle]
pub unsafe extern "C" fn rt_mutex_take_interruptible(mutex: *mut Mutex, time: i32) -> i32 {
    assert!(!mutex.is_null());
    (*mutex)
        .lock_internal(time, SuspendFlag::Interruptible)
        .map_or_else(|e| e.to_errno(), |_| code::EOK.to_errno())
}

#[no_mangle]
pub unsafe extern "C" fn rt_mutex_take_killable(mutex: *mut Mutex, time: i32) -> i32 {
    assert!(!mutex.is_null());
    (*mutex)
        .lock_internal(time, SuspendFlag::Killable)
        .map_or_else(|e| e.to_errno(), |_| code::EOK.to_errno())
}

#[no_mangle]
pub unsafe extern "C" fn rt_mutex_trytake(mutex: *mut Mutex) -> i32 {
    assert!(!mutex.is_null());
    (*mutex)
        .try_lock()
        .map_or_else(|e| e.to_errno(), |_| code::EOK.to_errno())
}

#[no_mangle]
pub unsafe extern "C" fn rt_mutex_release(mutex: *mut Mutex) -> i32 {
    assert!(!mutex.is_null());
    (*mutex)
        .unlock()
        .map_or_else(|e| e.to_errno(), |_| code::EOK.to_errno())
}
