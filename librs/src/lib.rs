#![cfg_attr(not(feature = "std"), no_std)]
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
    pub fn posix_memalign(memptr: *mut *mut u8, align: usize, size: usize) -> core::ffi::c_int;
    pub fn free(ptr: *mut u8);
}

// We don't expose any interfaces or types externally, rust-lang/libc is doing that.
pub mod errno;
pub mod iter;
pub mod pthread;
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
