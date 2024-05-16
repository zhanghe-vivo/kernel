#![warn(missing_docs)]
use core::{ffi, ptr};
use core::alloc::Layout;
use crate::rt_bindings::*;

// #[cfg(feature = "buddy")]
// pub use buddy::Heap as BuddyHeap;
#[cfg(feature = "llff")]
pub mod llff;
#[cfg(feature = "llff")]
pub use llff::Heap as Heap;
// #[cfg(feature = "tlsf")]
// pub use tlsf::Heap as TlsfHeap;

#[global_allocator]
static HEAP: Heap = Heap::empty();
#[cfg(feature = "RT_USING_HOOK")]
static mut RT_MALLOC_HOOK: Option<extern "C" fn(*mut ffi::c_void, usize)> = None;
#[cfg(feature = "RT_USING_HOOK")]
static mut RT_FREE_HOOK: Option<extern "C" fn(*mut ffi::c_void)> = None;

/// Align address and size downwards.
///
/// Returns the greatest `x` with alignment `align` so that `x <= addr`.
///
/// The alignment must be a power of two.
#[inline]
pub const fn align_down_size(addr: usize, align: usize) -> usize {
    addr & !(align - 1)
}

/// Align address and size upwards.
///
/// Returns the smallest `x` with alignment `align` so that `x >= addr`.
///
/// The alignment must be a power of two.
#[inline]
pub const fn align_up_size(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

/// Align upwards. Returns the smallest x with alignment `align`
/// so that x >= addr. The alignment must be a power of 2.
pub fn align_up(addr: *mut u8, align: usize) -> *mut u8 {
    let offset = addr.align_offset(align);
    addr.wrapping_add(offset)
}

/// Returns the offset of the address within the alignment.
///
/// Equivalent to `addr % align`, but the alignment must be a power of two.
#[inline]
pub const fn align_offset(addr: usize, align: usize) -> usize {
    addr & (align - 1)
}

/// Checks whether the address has the demanded alignment.
///
/// Equivalent to `addr % align == 0`, but the alignment must be a power of two.
#[inline]
pub const fn is_aligned(addr: usize, align: usize) -> bool {
    align_offset(addr, align) == 0
}

#[cfg(feature = "RT_USING_HOOK")]
#[no_mangle]
pub extern "C" fn rt_malloc_sethook(hook: extern "C" fn(*mut ffi::c_void, usize)) {
    unsafe { RT_MALLOC_HOOK = Some(hook) };
}

#[cfg(feature = "RT_USING_HOOK")]
#[no_mangle]
pub extern "C" fn rt_free_sethook(hook: extern "C" fn(*mut ffi::c_void)) {
    unsafe { RT_FREE_HOOK = Some(hook) };
}

#[no_mangle]
pub extern "C" fn rt_system_heap_init(begin_addr: *mut ffi::c_void, end_addr: *mut ffi::c_void) {
    // Initialize the allocator BEFORE you use it
    assert!(end_addr > begin_addr);
    let heap_size = end_addr as usize - begin_addr as usize;

    unsafe { HEAP.init(begin_addr as usize, heap_size) }
}

#[no_mangle]
pub extern "C" fn rt_malloc(size: usize) -> *mut ffi::c_void {
    if core::intrinsics::unlikely(size == 0) {
        return ptr::null_mut() as *mut ffi::c_void;
    }

    let layout = Layout::from_size_align(size, RT_ALIGN_SIZE as usize).unwrap();
    if let Some(alloc_ptr) = HEAP.alloc(layout) {
        alloc_ptr.as_ptr() as *mut ffi::c_void
    } else {
        kprintf!("no memory for size %u\n", size);
        ptr::null_mut() as *mut ffi::c_void
    }
}
#[no_mangle]
pub extern "C" fn rt_free(ptr: *mut ffi::c_void) {
    if core::intrinsics::unlikely(ptr.is_null()) {
        return;
    }

    let layout = Layout::from_size_align(0, RT_ALIGN_SIZE as usize).unwrap();
    unsafe { HEAP.dealloc(ptr as *mut u8, layout) };
}
#[no_mangle]
pub extern "C" fn rt_realloc(ptr: *mut ffi::c_void, newsize: usize) -> *mut ffi::c_void {
    if newsize == 0 {
        rt_free(ptr);
        return ptr::null_mut() as *mut ffi::c_void;
    }

    if ptr.is_null() {
        return rt_malloc(newsize);
    }
    
    let layout = Layout::from_size_align(0, RT_ALIGN_SIZE as usize).unwrap();
    if let Some(alloc_ptr) = unsafe { HEAP.realloc(ptr as *mut u8, layout, newsize) } {
        alloc_ptr.as_ptr() as *mut ffi::c_void
    } else {
        kprintf!("no memory for size %u\n", newsize);
        ptr::null_mut() as *mut ffi::c_void
    }
}
#[no_mangle]
pub extern "C" fn rt_calloc(count: usize, size: usize) -> *mut ffi::c_void {
    let required_size = count * size;
    let layout = Layout::from_size_align(required_size, RT_ALIGN_SIZE as usize).unwrap();
    if let Some(alloc_ptr) = HEAP.alloc(layout) {
        let c_ptr = alloc_ptr.as_ptr() as *mut ffi::c_void;
        unsafe {
            rt_memset(c_ptr, 0, required_size as u32);
        }
        c_ptr
    } else {
        kprintf!("no memory for size %u\n", required_size);
        ptr::null_mut() as *mut ffi::c_void
    }
}
#[no_mangle]
pub extern "C" fn rt_malloc_align(size: usize, align: usize) -> *mut ffi::c_void {
    let layout = Layout::from_size_align(size, align).unwrap();
    if let Some(alloc_ptr) = HEAP.alloc(layout) {
        alloc_ptr.as_ptr() as *mut ffi::c_void
    } else {
        kprintf!("no memory for size %u\n", size);
        ptr::null_mut() as *mut ffi::c_void
    }
}
#[no_mangle]
pub extern "C" fn rt_free_align(ptr: *mut ffi::c_void) {
    let layout = Layout::from_size_align(0, RT_ALIGN_SIZE as usize).unwrap();
    unsafe { HEAP.dealloc(ptr as *mut u8, layout) };
}
#[no_mangle]
pub extern "C" fn rt_memory_info(total: *mut usize, used: *mut usize, max_used: *mut usize) {
    let (total_size, used_size, required, maximum) = HEAP.memory_info();
    unsafe {
        *total = total_size;
        *used = used_size;
        *max_used = maximum;
    }
}