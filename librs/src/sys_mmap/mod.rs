use crate::{
    errno::{Errno, Result, SysCallFailed, ERRNO},
    pal::{Pal, Sys},
};
use libc::{c_int, c_void, off_t, size_t};

pub const PROT_READ: c_int = 0x0001;
pub const PROT_WRITE: c_int = 0x0002;
pub const PROT_EXEC: c_int = 0x0004;
pub const PROT_NONE: c_int = 0x0000;

pub const MAP_SHARED: c_int = 0x0001;
pub const MAP_PRIVATE: c_int = 0x0002;
pub const MAP_ANONYMOUS: c_int = 0x0020;

pub const MAP_FAILED: *mut c_void = usize::wrapping_neg(1) as *mut c_void;

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/mmap.html>.
/// This is not valid for blueos now , but is provided for malloc implementation.
#[no_mangle]
pub unsafe extern "C" fn mmap(
    addr: *mut c_void,
    len: size_t,
    prot: c_int,
    flags: c_int,
    fildes: c_int,
    off: off_t,
) -> *mut c_void {
    match Sys::mmap(addr, len, prot, flags, fildes, off) {
        Ok(ptr) => ptr,
        Err(Errno(errno)) => {
            ERRNO.set(errno);
            MAP_FAILED
        }
    }
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/munmap.html>.
/// This is not valid for blueos now , but is provided for compatibility with usermode applications.
#[no_mangle]
pub unsafe extern "C" fn munmap(addr: *mut c_void, len: size_t) -> c_int {
    Sys::munmap(addr, len).map(|()| 0).syscall_failed()
}
