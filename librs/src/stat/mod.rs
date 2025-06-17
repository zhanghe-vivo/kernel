use crate::{
    c_str::CStr,
    errno::SysCallFailed,
    syscall::{Sys, Syscall},
};
use libc::{c_char, c_int, dev_t, mode_t, statvfs};

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/chmod.html>.
#[no_mangle]
pub unsafe extern "C" fn chmod(path: *const c_char, mode: mode_t) -> c_int {
    let path = CStr::from_ptr(path);
    Sys::chmod(path, mode).map(|()| 0).syscall_failed()
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/fchmod.html>.
#[no_mangle]
pub extern "C" fn fchmod(fildes: c_int, mode: mode_t) -> c_int {
    Sys::fchmod(fildes, mode).map(|()| 0).syscall_failed()
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/fstat.html>.
#[no_mangle]
pub unsafe extern "C" fn fstat(fildes: c_int, buf: *mut c_char) -> c_int {
    Sys::fstat(fildes, buf).map(|()| 0).syscall_failed()
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/fstatvfs.html>.
#[no_mangle]
pub unsafe extern "C" fn fstatvfs(fildes: c_int, buf: *mut statvfs) -> c_int {
    Sys::fstatvfs(fildes, buf).map(|()| 0).syscall_failed()
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/statfs.html>.
#[no_mangle]
pub unsafe extern "C" fn statfs(path: *const c_char, buf: *mut c_char) -> c_int {
    let path = CStr::from_ptr(path);
    Sys::statfs(path, buf).map(|()| 0).syscall_failed()
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/mkdir.html>.
#[no_mangle]
pub unsafe extern "C" fn mkdir(path: *const c_char, mode: mode_t) -> c_int {
    let path = CStr::from_ptr(path);
    Sys::mkdir(path, mode).map(|()| 0).syscall_failed()
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/mkfifo.html>.
#[no_mangle]
pub unsafe extern "C" fn mkfifo(path: *const c_char, mode: mode_t) -> c_int {
    let path = CStr::from_ptr(path);
    Sys::mkfifo(path, mode).map(|()| 0).syscall_failed()
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/mknod.html>.
#[no_mangle]
pub unsafe extern "C" fn mknod(path: *const c_char, mode: mode_t, dev: dev_t) -> c_int {
    let path = CStr::from_ptr(path);
    Sys::mknod(path, mode, dev).map(|()| 0).syscall_failed()
}
