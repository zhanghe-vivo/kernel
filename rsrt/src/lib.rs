#![cfg_attr(not(feature = "std"), no_std)]
use core::alloc::{GlobalAlloc, Layout};

#[cfg(not(feature = "std"))]
struct PosixAllocator;

#[cfg(not(feature = "std"))]
unsafe impl GlobalAlloc for PosixAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() == 0 {
            return core::ptr::null_mut();
        }

        let mut mem_ptr: *mut libc::c_void = core::ptr::null_mut();
        let align = layout.align();
        let size = layout.size();

        let result = libc::posix_memalign(&mut mem_ptr as *mut *mut libc::c_void, align, size);

        if result == 0 {
            mem_ptr as *mut u8
        } else {
            core::ptr::null_mut()
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        if !ptr.is_null() {
            libc::free(ptr as *mut libc::c_void);
        }
    }
}

// rust-std has its own allocator and panic_handler.
#[cfg(not(feature = "std"))]
#[global_allocator]
static GLOBAL: PosixAllocator = PosixAllocator;

#[cfg(not(feature = "std"))]
#[panic_handler]
fn oops(info: &core::panic::PanicInfo) -> ! {
    // TODO: Use libc's printf.
    #[cfg(test)]
    {
        semihosting::println!("{}", info);
        semihosting::println!("{}", info.message());
    }
    loop {}
}

#[used]
#[link_section = ".bk_app_array"]
static INIT: extern "C" fn() = init;

extern "C" fn init() {
    #[cfg(feature = "std")]
    {
        extern "C" {
            fn __librs_start_main();
        }
        unsafe { __librs_start_main() };
    }

    #[cfg(not(feature = "std"))]
    {
        extern "C" {
            fn main() -> i32;
        }
        unsafe { main() };
    }
}
