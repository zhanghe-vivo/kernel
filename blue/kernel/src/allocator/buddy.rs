use crate::sync::{new_heaplock, HeapLock};
use core::alloc::Layout;
use core::pin::Pin;
use core::ptr::NonNull;
use pinned_init::*;

pub mod buddy_system_heap;
use buddy_system_heap::Heap as BuddyHeap;

/// A buddy system heap.
#[pin_data]
pub struct Heap {
    #[pin]
    heap: HeapLock<BuddyHeap<32>>,
}

impl Heap {
    /// Create a new UNINITIALIZED heap allocator, for global_allocator
    ///
    /// You must initialize this heap using the
    /// [`init`](Self::init) method before using the allocator.
    pub fn new() -> impl PinInit<Self> {
        pin_init!(Heap{
            heap <- new_heaplock!(BuddyHeap::empty(), "heap")
        })
    }

    /// Initializes the heap
    ///
    /// This function must be called BEFORE you run any code that makes use of the
    /// allocator.
    ///
    /// `start_addr` is the address where the heap will be located.
    ///
    /// `size` is the size of the heap in bytes.
    ///
    /// Note that:
    ///
    /// - The heap grows "upwards", towards larger addresses. Thus `start_addr` will
    ///   be the smallest address used.
    ///
    /// - The largest address used is `start_addr + size - 1`, so if `start_addr` is
    ///   `0x1000` and `size` is `0x30000` then the allocator won't use memory at
    ///   addresses `0x31000` and larger.
    ///
    /// # Safety
    ///
    /// Obey these or Bad Stuff will happen.
    ///
    /// - This function must be called exactly ONCE.
    /// - `size > 0`
    pub unsafe fn init(&self, start_addr: usize, size: usize) {
        let mut heap = self.heap.lock();
        (*heap).init(start_addr, size);
    }

    pub fn alloc(&self, layout: Layout) -> Option<NonNull<u8>> {
        let mut heap = self.heap.lock();
        let ptr = (*heap).allocate(&layout);
        ptr
    }

    pub unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut heap = self.heap.lock();
        (*heap).deallocate(NonNull::new_unchecked(ptr), &layout);
    }

    pub unsafe fn realloc(
        &self,
        ptr: *mut u8,
        layout: Layout,
        new_size: usize,
    ) -> Option<NonNull<u8>> {
        let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());
        let mut heap = self.heap.lock();
        let new_ptr = (*heap).reallocate(NonNull::new_unchecked(ptr), &new_layout);
        new_ptr
    }

    pub fn memory_info(&self) -> (usize, usize, usize) {
        let heap = self.heap.lock();
        let x = (
            (*heap).stats_total_bytes(),
            (*heap).stats_alloc_actual(),
            (*heap).stats_alloc_max(),
        );
        x
    }
}
