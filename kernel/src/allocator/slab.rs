use crate::sync::SpinLock;
use core::{alloc::Layout, ptr::NonNull};
pub mod slab_heap;
use crate::allocator::MemoryInfo;
use slab_heap::Heap as SlabHeap;

pub const PAGE_SIZE: usize = 4096;
pub const NUM_OF_SLABS: usize = 8;
pub const MIN_SLAB_SIZE: usize = PAGE_SIZE;
pub const MIN_HEAP_SIZE: usize = NUM_OF_SLABS * MIN_SLAB_SIZE;

/// A slab heap.
pub struct Heap {
    heap: SpinLock<SlabHeap>,
}

impl Heap {
    /// Create a new UNINITIALIZED heap allocator, for global_allocator
    ///
    /// You must initialize this heap using the
    /// [`init`](Self::init) method before using the allocator.
    pub const fn new() -> Self {
        Heap {
            heap: SpinLock::new(SlabHeap::empty()),
        }
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
        assert!(
            size >= MIN_HEAP_SIZE,
            "Heap size should be greater or equal to minimum heap size"
        );
        let mut heap = self.heap.lock_irqsave();
        (*heap).init(start_addr, size);
    }

    pub fn alloc(&self, layout: Layout) -> Option<NonNull<u8>> {
        let mut heap = self.heap.lock_irqsave();
        let ptr = (*heap).allocate(&layout);
        ptr
    }

    pub unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut heap = self.heap.lock_irqsave();
        (*heap).deallocate(NonNull::new_unchecked(ptr), &layout);
    }

    pub unsafe fn deallocate_unknown_align(&self, ptr: *mut u8) {
        let mut heap = self.heap.lock_irqsave();
        (*heap).deallocate_unknown_align(NonNull::new_unchecked(ptr));
    }

    pub unsafe fn realloc(
        &self,
        ptr: *mut u8,
        layout: Layout,
        new_size: usize,
    ) -> Option<NonNull<u8>> {
        let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());
        let mut heap = self.heap.lock_irqsave();
        let new_ptr = (*heap).reallocate(NonNull::new_unchecked(ptr), &new_layout);
        new_ptr
    }

    pub unsafe fn realloc_unknown_align(
        &self,
        ptr: *mut u8,
        new_size: usize,
    ) -> Option<NonNull<u8>> {
        let mut heap = self.heap.lock_irqsave();
        let new_ptr = (*heap).reallocate_unknown_align(NonNull::new_unchecked(ptr), new_size);
        new_ptr
    }

    pub fn memory_info(&self) -> MemoryInfo {
        let heap = self.heap.lock_irqsave();
        MemoryInfo {
            total: (*heap).total(),
            used: (*heap).allocated(),
            max_used: (*heap).maximum(),
        }
    }
}
