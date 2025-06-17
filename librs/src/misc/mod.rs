use crate::{
    c_str::CStr,
    errno::SysCallFailed,
    pal::{Pal, Sys},
};
use libc::{c_int, utsname};
#[no_mangle]
pub unsafe extern "C" fn uname(name: *mut utsname) -> c_int {
    Sys::uname(name).map(|()| 0).syscall_failed()
}
