//! Extensions to the [`alloc`] crate.

#![warn(missing_docs)]
use crate::{rt_bindings::*, static_init::UnsafeStaticInit};
use core::{
    alloc::{GlobalAlloc, Layout},
    ffi, ptr,
};
use pinned_init::PinInit;

#[cfg(feature = "slab")]
pub mod buddy;
#[cfg(feature = "slab")]
pub mod slab;
#[cfg(feature = "slab")]
pub use slab::Heap;
#[cfg(feature = "llff")]
pub mod llff;
#[cfg(feature = "llff")]
pub use llff::Heap;
#[cfg(feature = "buddy")]
pub mod buddy;
#[cfg(feature = "buddy")]
pub use buddy::Heap;
#[cfg(feature = "tlsf")]
pub mod tlsf;
#[cfg(feature = "tlsf")]
pub use tlsf::Heap;

mod block_hdr;
mod int;
mod utils;

struct KernelAllocator;

struct HeapInit;

unsafe impl PinInit<Heap> for HeapInit {
    unsafe fn __pinned_init(self, slot: *mut Heap) -> Result<(), core::convert::Infallible> {
        let init = Heap::new();
        unsafe { init.__pinned_init(slot) }
    }
}

static HEAP: UnsafeStaticInit<Heap, HeapInit> = UnsafeStaticInit::new(HeapInit);

#[global_allocator]
static ALLOCATOR: KernelAllocator = KernelAllocator;
#[cfg(feature = "RT_USING_HOOK")]
static mut RT_MALLOC_HOOK: Option<extern "C" fn(*mut ffi::c_void, usize)> = None;
#[cfg(feature = "RT_USING_HOOK")]
static mut RT_FREE_HOOK: Option<extern "C" fn(*mut ffi::c_void)> = None;

/// rt_align!(size, align)
///
/// the most contiguous size aligned at specified width.
///
#[macro_export]
macro_rules! rt_align {
    ($size:expr, $align:expr) => {
        ($size + $align - 1) & !($align - 1)
    };
}

/// impl for GlobalAlloc and allocator_api
unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        HEAP.alloc(layout)
            .map_or(ptr::null_mut(), |allocation| allocation.as_ptr())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        HEAP.dealloc(ptr, layout);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        HEAP.realloc(ptr, layout, new_size)
            .map_or(ptr::null_mut(), |allocation| allocation.as_ptr())
    }
}

#[cfg(feature = "allocator_api")]
mod allocator_api {
    use super::*;
    use core::alloc::{AllocError, Allocator};

    unsafe impl Allocator for KernelAllocator {
        fn allocate(&self, layout: Layout) -> Result<ptr::NonNull<[u8]>, AllocError> {
            match layout.size() {
                0 => Ok(ptr::NonNull::slice_from_raw_parts(layout.dangling(), 0)),
                size => HEAP.alloc(layout).map_or(Err(AllocError), |allocation| {
                    Ok(ptr::NonNull::slice_from_raw_parts(allocation, size))
                }),
            }
        }
        unsafe fn deallocate(&self, ptr: ptr::NonNull<u8>, layout: Layout) {
            if layout.size() != 0 {
                HEAP.dealloc(ptr.as_ptr(), layout);
            }
        }
    }
}

/// Align address and size downwards.
///
/// Returns the greatest `x` with alignment `align` so that `x <= addr`.
///
/// The alignment must be a power of two.
#[allow(dead_code)]
#[inline]
pub const fn align_down_size(addr: usize, align: usize) -> usize {
    addr & !(align - 1)
}

/// Align address and size upwards.
///
/// Returns the smallest `x` with alignment `align` so that `x >= addr`.
///
/// The alignment must be a power of two.
#[allow(dead_code)]
#[inline]
pub const fn align_up_size(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

/// Align upwards. Returns the smallest x with alignment `align`
/// so that x >= addr. The alignment must be a power of 2.
#[allow(dead_code)]
#[inline]
pub fn align_up(addr: *mut u8, align: usize) -> *mut u8 {
    align_up_size(addr as usize, align) as *mut u8
}

/// Returns the offset of the address within the alignment.
///
/// Equivalent to `addr % align`, but the alignment must be a power of two.
#[allow(dead_code)]
#[inline]
pub const fn align_offset(addr: usize, align: usize) -> usize {
    addr & (align - 1)
}

/// Checks whether the address has the demanded alignment.
///
/// Equivalent to `addr % align == 0`, but the alignment must be a power of two.
#[allow(dead_code)]
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
pub unsafe extern "C" fn rt_system_heap_init(
    begin_addr: *mut ffi::c_void,
    end_addr: *mut ffi::c_void,
) {
    // Initialize the allocator BEFORE you use it
    assert!(end_addr > begin_addr);
    let heap_size = end_addr as usize - begin_addr as usize;
    HEAP.init_once();
    HEAP.init(begin_addr as usize, heap_size);
}

#[no_mangle]
pub unsafe extern "C" fn rt_malloc(size: usize) -> *mut ffi::c_void {
    if core::intrinsics::unlikely(size == 0) {
        return ptr::null_mut() as *mut ffi::c_void;
    }

    let layout = Layout::from_size_align(size, RT_ALIGN_SIZE as usize).unwrap();
    let ptr = HEAP
        .alloc(layout)
        .map_or(ptr::null_mut(), |allocation| allocation.as_ptr());
    ptr as *mut ffi::c_void
}
#[no_mangle]
pub unsafe extern "C" fn rt_free(ptr: *mut ffi::c_void) {
    if core::intrinsics::unlikely(ptr.is_null()) {
        return;
    }

    let layout = Layout::from_size_align(0, RT_ALIGN_SIZE as usize).unwrap();
    HEAP.dealloc(ptr as *mut u8, layout);
}
#[no_mangle]
pub unsafe extern "C" fn rt_realloc(ptr: *mut ffi::c_void, newsize: usize) -> *mut ffi::c_void {
    if newsize == 0 {
        rt_free(ptr);
        return ptr::null_mut() as *mut ffi::c_void;
    }

    if ptr.is_null() {
        return rt_malloc(newsize);
    }

    let layout = Layout::from_size_align(0, RT_ALIGN_SIZE as usize).unwrap();
    let ptr = HEAP
        .realloc(ptr as *mut u8, layout, newsize)
        .map_or(ptr::null_mut(), |allocation| allocation.as_ptr());
    ptr as *mut ffi::c_void
}
#[no_mangle]
pub unsafe extern "C" fn rt_calloc(count: usize, size: usize) -> *mut ffi::c_void {
    let required_size = count * size;
    let layout = Layout::from_size_align(required_size, RT_ALIGN_SIZE as usize).unwrap();
    if let Some(alloc_ptr) = HEAP.alloc(layout) {
        ptr::write_bytes(alloc_ptr.as_ptr(), 0, required_size);
        alloc_ptr.as_ptr() as *mut ffi::c_void
    } else {
        ptr::null_mut() as *mut ffi::c_void
    }
}
#[no_mangle]
pub unsafe extern "C" fn rt_malloc_align(size: usize, align: usize) -> *mut ffi::c_void {
    if core::intrinsics::unlikely(size == 0) {
        return ptr::null_mut() as *mut ffi::c_void;
    }

    let layout = Layout::from_size_align(size, align).unwrap();
    let ptr = HEAP
        .alloc(layout)
        .map_or(ptr::null_mut(), |allocation| allocation.as_ptr());
    ptr as *mut ffi::c_void
}
#[no_mangle]
pub unsafe extern "C" fn rt_free_align(ptr: *mut ffi::c_void) {
    let layout = Layout::from_size_align(0, RT_ALIGN_SIZE as usize).unwrap();
    HEAP.dealloc(ptr as *mut u8, layout);
}
#[no_mangle]
pub extern "C" fn rt_memory_info(total: *mut usize, used: *mut usize, max_used: *mut usize) {
    let (total_size, used_size, maximum) = HEAP.memory_info();
    unsafe {
        *total = total_size;
        *used = used_size;
        *max_used = maximum;
    }
}
