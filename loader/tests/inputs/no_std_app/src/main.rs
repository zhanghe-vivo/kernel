#![no_main]
#![no_std]
#![feature(thread_local)]
#![feature(rustc_private)]
#![feature(fn_align)]

extern crate alloc;
use alloc::{boxed::Box, ffi::CString, format, vec::Vec};
use core::{
    alloc::{GlobalAlloc, Layout},
    ffi::CStr,
    ptr,
};
use libc::c_void;

#[cfg(coverage)]
use common_cov;

#[thread_local]
static mut ID: i32 = 42;

// Should be put in .bss section.
static mut LARGE_ARRAY: [u8; 1024] = [0u8; 1024];

struct PosixAllocator;

unsafe impl GlobalAlloc for PosixAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() == 0 {
            return ptr::null_mut();
        }

        let mut mem_ptr: *mut c_void = ptr::null_mut();
        let align = layout.align();
        let size = layout.size();

        let result =
            librs::stdlib::malloc::posix_memalign(&mut mem_ptr as *mut *mut c_void, align, size);

        if result == 0 {
            mem_ptr as *mut u8
        } else {
            ptr::null_mut()
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        if !ptr.is_null() {
            librs::stdlib::malloc::free(ptr as *mut c_void);
        }
    }
}

#[global_allocator]
static GLOBAL: PosixAllocator = PosixAllocator;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    loop {}
}

#[no_mangle]
#[repr(align(8))]
pub extern "C" fn _start() {
    assert_eq!(main(42), 42);
}

#[no_mangle]
#[inline(never)]
extern "C" fn main(argc: i32) -> i32 {
    // FIXME: Current librs::stdio::puts impl makes this program hangs forever.
    unsafe {
        let b = Box::new(ID);
        let mut v = Vec::new();
        v.extend_from_slice(&[ID; 128]);
        LARGE_ARRAY[argc as usize] = ID as u8;
        assert_eq!(*b as u8, LARGE_ARRAY[argc as usize]);

        #[cfg(coverage)]
        common_cov::write_coverage_data();

        return LARGE_ARRAY[argc as usize] as i32;
    }
}
