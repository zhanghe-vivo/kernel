// NEWLINE-TIMEOUT: 10
// ASSERT-SUCC: Librs integration test ended
// ASSERT-FAIL: Backtrace in Panic.*
#![no_main]
#![no_std]
#![feature(custom_test_frameworks)]
#![test_runner(librs_test_runner)]
#![reexport_test_harness_main = "librs_test_main"]
#![feature(c_size_t)]
#![feature(thread_local)]

use bluekernel::{allocator, println, thread::Thread};
use core::ffi::c_void;
use libc::pthread_t;
use librs::pthread::{pthread_create, pthread_join};

mod ctype;
mod pthread;
mod scal;
mod semihosting;

pub fn librs_test_runner(tests: &[&dyn Fn()]) {
    println!("Librs integration test started");
    println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
    println!("Librs integration test ended");
}

extern "C" fn posix_main(_: *mut c_void) -> *mut c_void {
    librs_test_main();
    core::ptr::null_mut()
}

#[no_mangle]
fn main() -> i32 {
    // We must enter POSIX subsystem first to perform pthread testing.
    let mut t: pthread_t = 0;
    let rc = pthread_create(
        &mut t as *mut pthread_t,
        core::ptr::null(),
        posix_main,
        core::ptr::null_mut(),
    );
    assert_eq!(rc, 0);
    pthread_join(t, core::ptr::null_mut());
    0
}
