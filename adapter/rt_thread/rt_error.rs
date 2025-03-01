use crate::bluekernel::error;

#[no_mangle]
pub unsafe extern "C" fn rt_strerror(error: i32) -> *const core::ffi::c_char {
    error::strerror(error)
}

#[no_mangle]
pub unsafe extern "C" fn rt_get_errno() -> i32 {
    error::get_errno()
}

#[no_mangle]
pub unsafe extern "C" fn rt_set_errno(error: i32) {
    error::set_errno(error);
}

#[no_mangle]
pub unsafe extern "C" fn _rt_errno() -> *mut i32 {
    error::_errno()
}
