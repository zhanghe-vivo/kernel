use crate::{
    c_str::CStr,
    sys_mmap::{mmap, munmap, MAP_ANONYMOUS, MAP_PRIVATE, PROT_READ, PROT_WRITE},
};
use core::{
    alloc::{GlobalAlloc, Layout},
    ffi::c_void,
    ptr, str,
};

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
        // memory don't need to be deallocated in linux emulation
    }
}

#[global_allocator]
static GLOBAL: SimpleAllocator = SimpleAllocator;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    loop {}
}
