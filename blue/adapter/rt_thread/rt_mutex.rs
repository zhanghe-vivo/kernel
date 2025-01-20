use crate::blue_kernel::{error::code, sync::lock::mutex::Mutex, thread::SuspendFlag};
use core::ffi;

#[no_mangle]
pub unsafe extern "C" fn rt_mutex_init(
    mutex: *mut Mutex,
    name: *const ffi::c_char,
    _flag: ffi::c_uchar,
) -> i32 {
    assert!(!mutex.is_null());

    (*mutex).init(name, _flag);

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
    Mutex::new_raw(name, _flag)
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
    (*mutex).lock_wait(time)
}

#[no_mangle]
pub unsafe extern "C" fn rt_mutex_take_interruptible(mutex: *mut Mutex, time: i32) -> i32 {
    assert!(!mutex.is_null());
    (*mutex).lock_internal(time, SuspendFlag::Interruptible as u32)
}

#[no_mangle]
pub unsafe extern "C" fn rt_mutex_take_killable(mutex: *mut Mutex, time: i32) -> i32 {
    assert!(!mutex.is_null());
    (*mutex).lock_internal(time, SuspendFlag::Killable as u32)
}

#[no_mangle]
pub unsafe extern "C" fn rt_mutex_trytake(mutex: *mut Mutex) -> i32 {
    assert!(!mutex.is_null());
    (*mutex).try_lock()
}

#[no_mangle]
pub unsafe extern "C" fn rt_mutex_release(mutex: *mut Mutex) -> i32 {
    assert!(!mutex.is_null());
    (*mutex).unlock()
}
