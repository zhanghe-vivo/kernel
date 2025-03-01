use crate::bluekernel::allocator;
use core::ffi;

#[no_mangle]
pub unsafe extern "C" fn rt_malloc(size: usize) -> *mut ffi::c_void {
    allocator::malloc(size) as *mut ffi::c_void
}

#[no_mangle]
pub unsafe extern "C" fn rt_free(ptr: *mut ffi::c_void) {
    allocator::free(ptr as *mut u8);
}

#[no_mangle]
pub unsafe extern "C" fn rt_realloc(ptr: *mut ffi::c_void, newsize: usize) -> *mut ffi::c_void {
    allocator::realloc(ptr as *mut u8, newsize) as *mut ffi::c_void
}

#[no_mangle]
pub unsafe extern "C" fn rt_calloc(count: usize, size: usize) -> *mut ffi::c_void {
    allocator::calloc(count, size) as *mut ffi::c_void
}

#[no_mangle]
pub unsafe extern "C" fn rt_malloc_align(size: usize, align: usize) -> *mut ffi::c_void {
    allocator::malloc_align(size, align) as *mut ffi::c_void
}

#[no_mangle]
pub unsafe extern "C" fn rt_free_align(ptr: *mut ffi::c_void) {
    allocator::free_align(ptr as *mut u8);
}

#[no_mangle]
pub extern "C" fn rt_memory_info(total: *mut usize, used: *mut usize, max_used: *mut usize) {
    allocator::memory_info(total, used, max_used);
}

#[no_mangle]
pub extern "C" fn rt_system_heap_init(
    begin_addr: *mut core::ffi::c_void,
    end_addr: *mut core::ffi::c_void,
) {
    allocator::system_heap_init(begin_addr as usize, end_addr as usize)
}
