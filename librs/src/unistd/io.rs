use bluekernel_header::syscalls::NR::{Close, Read, Write};
use bluekernel_scal::bk_syscall;
use libc::{c_int, c_ulong, c_void, size_t, ssize_t};

#[no_mangle]
#[linkage = "weak"]
pub extern "C" fn write(fd: i32, buf: *const u8, size: usize) -> isize {
    bk_syscall!(Write, fd, buf, size) as isize
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/close.html>.
#[no_mangle]
pub extern "C" fn close(fildes: c_int) -> c_int {
    bk_syscall!(Close, fildes) as c_int
}

/// See https://pubs.opengroup.org/onlinepubs/9799919799/functions/read.html
#[no_mangle]
pub unsafe extern "C" fn read(fildes: c_int, buf: *const c_void, nbyte: size_t) -> ssize_t {
    bk_syscall!(Read, fildes, buf as *mut c_void, nbyte) as ssize_t
}
