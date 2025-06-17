use crate::{
    c_str::CStr,
    sys_mmap::{mmap, munmap, MAP_ANONYMOUS, MAP_PRIVATE, PROT_READ, PROT_WRITE},
};
use core::{
    alloc::{GlobalAlloc, Layout},
    ffi::c_void,
    ptr, str,
};
use libc::{c_char, c_int, c_uint, pthread_t};
#[no_mangle]
pub unsafe extern "C" fn perror(s: *const c_char) {
    // todo : perror output to stderr
    match CStr::from_nullable_ptr(s).and_then(|s_cstr| str::from_utf8(s_cstr.to_bytes()).ok()) {
        Some(s_str) if !s_str.is_empty() => {
            // println!("{}", s_str)
        }
        _ => {
            // println!("{}", "Unknown error")
        }
    }
}

struct SimpleAllocator;

unsafe impl GlobalAlloc for SimpleAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() == 0 {
            return ptr::null_mut();
        }

        let align = layout.align();
        let size = layout.size();

        let result = mmap(
            core::ptr::null_mut(),
            size,
            PROT_READ | PROT_WRITE,
            MAP_PRIVATE | MAP_ANONYMOUS,
            -1,
            0,
        );
        if result.is_null() {
            return ptr::null_mut();
        }
        result as *mut u8
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        // memory don't need to be deallocated in usermode
    }
}

#[global_allocator]
static GLOBAL: SimpleAllocator = SimpleAllocator;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    loop {}
}
