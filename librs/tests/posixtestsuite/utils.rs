use crate::{
    c_str::CStr,
    stdio::{stderr, stdin, stdout},
};
use bluekernel::{println, thread::Thread};
use core::str;
use libc::{c_char, c_int, c_uint, pthread_t};

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
        // TODO: move to c runtime?
        io_init();
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

    #[cfg(coverage)]
    {
        use bluekernel::cov;
        cov::write_coverage_data();
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn exit(status: c_int) -> ! {
    use crate::pthread::pthread_exit;
    // exit will terminate whole process, now we just call pthread_exit instead
    pthread_exit(0 as *mut core::ffi::c_void);
}

fn io_init() {
    unsafe {
        stdin = crate::stdio::default_stdin().get();
        stdout = crate::stdio::default_stdout().get();
        stderr = crate::stdio::default_stderr().get();
    }
}
