use crate::c_str::CStr;
use bluekernel::{println, thread::Thread};
use core::str;
use libc::{c_char, c_int, c_uint, pthread_t};
// todo: printf is a very complex function, the call environment need stdout/va_args ,and  file system syscall implementation,
// and many other things. for now, we just stub it out. posix testsuite use it for output test information. we have
// [test] attribute do same things
#[no_mangle]
pub unsafe extern "C" fn printf(format: *const c_char, mut __valist: ...) -> c_int {
    0
}

// for posix testsuite, we need to implement a stub for perror
#[no_mangle]
pub unsafe extern "C" fn perror(s: *const c_char) {
    // todo : perror output to stderr
    match CStr::from_nullable_ptr(s).and_then(|s_cstr| str::from_utf8(s_cstr.to_bytes()).ok()) {
        Some(s_str) if !s_str.is_empty() => {
            println!("{}", s_str)
        }
        _ => {
            println!("{}", "Unknown error")
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn puts(s: *const c_char) -> c_int {
    perror(s);
    0
}

#[no_mangle]
pub extern "C" fn sleep(seconds: c_uint) -> c_uint {
    const TICK_PER_SECOND: u32 = 100;
    Thread::sleep(seconds * TICK_PER_SECOND);
    0
}

#[no_mangle]
pub unsafe extern "C" fn pthread_cancel(thread: pthread_t) -> c_int {
    0
}
