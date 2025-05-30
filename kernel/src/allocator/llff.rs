use crate::sync::SpinLock;
use core::{alloc::Layout, ptr::NonNull};
pub mod linked_list_heap;
use linked_list_heap::Heap as LLHeap;

/// A linked list first fit heap.
pub struct Heap {
    heap: SpinLock<LLHeap>,
}

impl Heap {
    /// Create a new UNINITIALIZED heap allocator
    ///
    /// You must initialize this heap using the
    /// [`init`](Self::init) method before using the allocator.
    pub const fn new() -> Self {
        Heap {
            heap: SpinLock::new(LLHeap::empty()),
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
        let mut heap = self.heap.lock_irqsave();
        (*heap).init(start_addr, size);
    }

    pub fn alloc(&self, layout: Layout) -> Option<NonNull<u8>> {
        let mut heap = self.heap.lock_irqsave();
        let ptr = (*heap).allocate_first_fit(&layout);
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
        let mut heap = self.heap.lock_irqsave();
        let new_ptr = (*heap).realloc(NonNull::new_unchecked(ptr), &layout, new_size);
        new_ptr
    }

    pub unsafe fn realloc_unknown_align(
        &self,
        ptr: *mut u8,
        new_size: usize,
    ) -> Option<NonNull<u8>> {
        let mut heap = self.heap.lock_irqsave();
        let new_ptr = (*heap).realloc_unknown_align(NonNull::new_unchecked(ptr), new_size);
        new_ptr
    }

    pub fn memory_info(&self) -> (usize, usize, usize) {
        let heap = self.heap.lock_irqsave();
        let x = ((*heap).total(), (*heap).allocated(), (*heap).maximum());
        x
    }
}
