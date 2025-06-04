//! Extensions to the [`alloc`] crate.

#![warn(missing_docs)]
use crate::{bluekernel_kconfig::ALIGN_SIZE, static_init::UnsafeStaticInit};
use core::{
    alloc::{GlobalAlloc, Layout},
    ptr,
};

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
/// using tlsf
pub mod tlsf;
#[cfg(feature = "tlsf")]
pub use tlsf::Heap;

mod block_hdr;
mod int;
mod utils;

struct KernelAllocator;
static HEAP: Heap = Heap::new();

#[global_allocator]
static ALLOCATOR: KernelAllocator = KernelAllocator;

/// impl for GlobalAlloc and allocator_api
unsafe impl GlobalAlloc for KernelAllocator {
    /// try to allocate memory with the given layout
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        HEAP.alloc(layout)
            .map_or(ptr::null_mut(), |ptr| ptr.as_ptr())
    }

    /// deallocate the memory pointed by ptr with the given layout
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        HEAP.dealloc(ptr, layout);
    }

    /// reallocate memory with the given size and layout
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        HEAP.realloc(ptr, layout, new_size)
            .map_or(ptr::null_mut(), |ptr| ptr.as_ptr())
    }
}

#[cfg(feature = "allocator_api")]
mod allocator_api {
    use super::{ptr, KernelAllocator, Layout, HEAP};
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

/// Initialize the system heap with given begin address and end address.
///
/// # Arguments
///
/// * `begin_addr` - The beginning address of the heap.
/// * `end_addr` - The ending address of the heap.
pub fn system_heap_init(begin_addr: usize, end_addr: usize) {
    // Initialize the allocator BEFORE you use it
    assert!(end_addr > begin_addr);
    let heap_size = end_addr - begin_addr;
    unsafe {
        HEAP.init(begin_addr, heap_size);
    }
}

/// Allocate memory on heap and returns a pointer to it.
/// If size equals zero, then null mutable raw pointer will be returned.
pub fn malloc(size: usize) -> *mut u8 {
    if core::intrinsics::unlikely(size == 0) {
        return ptr::null_mut();
    }
    let layout = Layout::from_size_align(size, ALIGN_SIZE).unwrap();
    let ptr = HEAP
        .alloc(layout)
        .map_or(ptr::null_mut(), |allocation| allocation.as_ptr());
    ptr
}

/// Free previously allocated memory pointed by ptr.
///
/// # Arguments
///
/// * `ptr` - A pointer pointing to the memory location to be freed.
pub fn free(ptr: *mut u8) {
    if core::intrinsics::unlikely(ptr.is_null()) {
        return;
    }
    unsafe { HEAP.deallocate_unknown_align(ptr) };
}

/// Reallocate memory pointed by ptr to have a new size.
///
/// # Arguments
///
/// * `ptr` - A pointer pointing to the memory location to be reallocated.
/// * `newsize` - The new size for the reallocated memory.
pub fn realloc(ptr: *mut u8, newsize: usize) -> *mut u8 {
    if newsize == 0 {
        free(ptr);
        return ptr::null_mut();
    }
    if ptr.is_null() {
        return malloc(newsize);
    }
    let ptr = unsafe {
        HEAP.realloc_unknown_align(ptr, newsize)
            .map_or(ptr::null_mut(), |ptr| ptr.as_ptr())
    };
    ptr
}

/// Allocates memory for an array of elements and initializes all bytes in this block to zero.
///
/// # Arguments
///
/// * `count` - Number of elements to allocate space for.
/// * `size` - Size of each element.
pub fn calloc(count: usize, size: usize) -> *mut u8 {
    let required_size = count * size;
    let layout = Layout::from_size_align(required_size, ALIGN_SIZE).unwrap();
    if let Some(alloc_ptr) = HEAP.alloc(layout) {
        unsafe { ptr::write_bytes(alloc_ptr.as_ptr(), 0, required_size) };
        alloc_ptr.as_ptr()
    } else {
        ptr::null_mut()
    }
}

/// Allocates aligned memory of at least the specified size.
///
/// # Arguments
///
/// * `size` - Minimum size of the memory region to allocate.
/// * `align` - Alignment requirement for the returned memory.
pub fn malloc_align(size: usize, align: usize) -> *mut u8 {
    if core::intrinsics::unlikely(size == 0) {
        return ptr::null_mut();
    }

    let layout = Layout::from_size_align(size, align.max(ALIGN_SIZE)).unwrap();
    let ptr = HEAP
        .alloc(layout)
        .map_or(ptr::null_mut(), |allocation| allocation.as_ptr());
    ptr
}

/// Deallocates memory that was allocated using `malloc_align`.
///
/// # Arguments
///
/// * `ptr` - Pointer to the memory region to deallocate.
pub fn free_align(ptr: *mut u8, align: usize) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let layout = Layout::from_size_align_unchecked(0, align);
        HEAP.dealloc(ptr, layout);
    }
}

/// Retrieves various statistics about the current state of the heap's memory usage.
///
/// # Arguments
///
/// * `total` - Output parameter containing the total available memory on the heap.
/// * `used` - Output parameter containing the currently used memory on the heap.
/// * `max_used` - Output parameter containing the largest amount of memory ever used during execution.
pub fn memory_info() -> (usize, usize, usize) {
    HEAP.memory_info()
}

mod ffi {
    use core::ffi::c_int;

    #[no_mangle]
    #[linkage = "weak"]
    pub extern "C" fn posix_memalign(ptr: *mut *mut u8, align: usize, size: usize) -> c_int {
        let addr = super::malloc_align(size, align);
        if addr.is_null() {
            return -1;
        }
        unsafe { *ptr = addr };
        0
    }

    #[no_mangle]
    #[linkage = "weak"]
    pub extern "C" fn free(ptr: *mut u8) {
        super::free(ptr)
    }

    #[no_mangle]
    #[linkage = "weak"]
    pub extern "C" fn malloc(size: usize) -> *mut u8 {
        super::malloc(size)
    }

    #[no_mangle]
    #[linkage = "weak"]
    pub extern "C" fn memalign(align: usize, size: usize) -> *mut u8 {
        super::malloc_align(size, align)
    }

    #[no_mangle]
    #[linkage = "weak"]
    pub extern "C" fn calloc(count: usize, size: usize) -> *mut u8 {
        super::calloc(count, size)
    }

    #[no_mangle]
    #[linkage = "weak"]
    pub extern "C" fn realloc(ptr: *mut u8, newsize: usize) -> *mut u8 {
        super::realloc(ptr, newsize)
    }
}
