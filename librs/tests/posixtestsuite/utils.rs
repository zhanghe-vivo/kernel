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

extern "C" fn posix_testsuite_main(_: *mut core::ffi::c_void) -> *mut core::ffi::c_void {
    extern "C" {
        fn main() -> i32;
    }
    unsafe {
        main();
    }
    core::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn start_posix_testsuite() -> i32 {
    use crate::pthread::{pthread_create, pthread_join};
    use libc::pthread_t;
    // We must enter POSIX subsystem first to perform posix testsuite testing.
    let mut t: pthread_t = 0;
    let rc = pthread_create(
        &mut t as *mut pthread_t,
        core::ptr::null(),
        posix_testsuite_main,
        core::ptr::null_mut(),
    );
    assert_eq!(rc, 0);
    pthread_join(t, core::ptr::null_mut());
    0
}

#[no_mangle]
pub unsafe extern "C" fn exit(status: c_int) -> ! {
    use crate::pthread::pthread_exit;
    // exit will terminate whole process, now we just call pthread_exit instead
    pthread_exit(0 as *mut core::ffi::c_void);
}
