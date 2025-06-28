#![deny(unsafe_op_in_unsafe_fn)]

#[allow(unused_imports)]
use bluekernel_header::syscalls::NR::{Fcntl, Open};
use bluekernel_scal::bk_syscall;
#[allow(unused_imports)]
use libc::{
    c_char, c_int, c_long, c_ulonglong, mode_t, F_DUPFD, F_GETLK, F_SETFD, F_SETFL, F_SETLK,
    F_SETLKW, O_CREAT, O_TRUNC, O_WRONLY,
};

#[no_mangle]
pub unsafe extern "C" fn creat(path: *const c_char, mode: mode_t) -> c_int {
    unsafe { open(path, O_WRONLY | O_CREAT | O_TRUNC, mode) }
}

#[no_mangle]
pub unsafe extern "C" fn fcntl(fildes: c_int, cmd: c_int, mut __valist: ...) -> c_int {
    // c_ulonglong
    let arg = match cmd {
        F_DUPFD | F_SETFD | F_SETFL | F_SETLK | F_SETLKW | F_GETLK => unsafe {
            __valist.arg::<c_ulonglong>()
        },
        _ => 0,
    };

    bk_syscall!(Fcntl, fildes, cmd, arg as usize) as c_int
}

// https://pubs.opengroup.org/onlinepubs/9799919799/functions/open.html
#[no_mangle]
pub unsafe extern "C" fn open(path: *const c_char, oflag: c_int, mut __valist: ...) -> c_int {
    let mode = if oflag & O_CREAT == O_CREAT {
        // SAFETY: The caller must ensure that the mode is valid.
        // We assume that the caller has passed a valid mode_t value.
        // The actual value of mode is extracted from the variadic arguments.

        unsafe { __valist.arg::<mode_t>() }
    } else {
        0
    };

    bk_syscall!(Open, path, oflag, mode) as c_int
}
