// NEWLINE-TIMEOUT: 10
// ASSERT-SUCC: Librs unittest ended
// ASSERT-FAIL: Backtrace in Panic.*

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(test, feature(custom_test_frameworks))]
#![cfg_attr(test, test_runner(librs_test_runner))]
#![cfg_attr(test, reexport_test_harness_main = "librs_test_main")]
#![cfg_attr(test, no_main)]
#![allow(internal_features)]
#![feature(c_size_t)]
#![feature(slice_internals)]
#![feature(ptr_as_uninit)]
#![feature(linkage)]
#![feature(lang_items)]
#![feature(thread_local)]
#![feature(box_as_ptr)]
#![feature(atomic_from_mut)]

extern crate alloc;
// TODO: Move to mman. We use them to allocate tls object.
extern "C" {
    pub fn malloc(size: usize) -> *mut core::ffi::c_void;
    pub fn posix_memalign(memptr: *mut *mut u8, align: usize, size: usize) -> core::ffi::c_int;
    pub fn free(ptr: *mut u8);
}

// We don't expose any interfaces or types externally, rust-lang/libc is doing that.
pub mod c_str;
pub mod ctype;
pub mod errno;
pub mod iter;
pub mod pthread;
pub mod stdlib;
pub mod string;
pub mod sync;
pub mod tls;
pub mod types;

#[cfg(target_arch = "arm")]
#[no_mangle]
#[linkage = "weak"]
pub unsafe extern "C" fn __aeabi_unwind_cpp_pr0() -> () {
    panic!("Unwind not implemented")
}

#[no_mangle]
#[linkage = "weak"]
pub unsafe extern "C" fn _Unwind_Backtrace(
    _trace: *mut core::ffi::c_void,
    _arg: *mut core::ffi::c_void,
) -> core::ffi::c_int {
    todo!()
}

#[no_mangle]
#[linkage = "weak"]
pub unsafe extern "C" fn _Unwind_GetIP(_context: *mut core::ffi::c_void) -> core::ffi::c_int {
    todo!()
}

#[cfg(test)]
use bluekernel::println;

#[cfg(test)]
pub fn librs_test_runner(tests: &[&dyn Fn()]) {
    println!("Librs unittest started");
    println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
    println!("Librs unittest ended");
}

#[cfg(test)]
extern "C" fn posix_main(_: *mut core::ffi::c_void) -> *mut core::ffi::c_void {
    librs_test_main();
    core::ptr::null_mut()
}

#[cfg(test)]
#[no_mangle]
fn main() -> i32 {
    use libc::pthread_t;
    use pthread::{pthread_create, pthread_join};
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
