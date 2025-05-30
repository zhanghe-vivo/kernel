use crate::sync::SpinLock;
use const_default::ConstDefault;
use core::{alloc::Layout, ptr::NonNull};
pub mod tlsf_heap;
use tlsf_heap::Tlsf;

type TlsfHeap = Tlsf<'static, usize, usize, { usize::BITS as usize }, { usize::BITS as usize }>;

/// A two-Level segregated fit heap.
pub struct Heap {
    heap: SpinLock<TlsfHeap>,
}

impl Heap {
    /// Create a new UNINITIALIZED heap allocator
    ///
    /// You must initialize this heap using the
    /// [`init`](Self::init) method before using the allocator.
    pub const fn new() -> Self {
        Heap {
            heap: SpinLock::new(ConstDefault::DEFAULT),
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
        let block: &[u8] = core::slice::from_raw_parts(start_addr as *const u8, size);
        let mut heap = self.heap.lock_irqsave();
        (*heap).insert_free_block_ptr(block.into());
    }

    /// try to allocate memory with the given layout
    pub fn alloc(&self, layout: Layout) -> Option<NonNull<u8>> {
        let mut heap = self.heap.lock_irqsave();
        let ptr = (*heap).allocate(&layout);
        ptr
    }

    /// deallocate the memory pointed by ptr with the given layout
    pub unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut heap = self.heap.lock_irqsave();
        (*heap).deallocate(NonNull::new_unchecked(ptr), layout.align());
    }

    pub unsafe fn deallocate_unknown_align(&self, ptr: *mut u8) {
        let mut heap = self.heap.lock_irqsave();
        (*heap).deallocate_unknown_align(NonNull::new_unchecked(ptr));
    }

    /// reallocate memory with the given size and layout
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

    /// reallocate memory with the given size but with out align
    pub unsafe fn realloc_unknown_align(
        &self,
        ptr: *mut u8,
        new_size: usize,
    ) -> Option<NonNull<u8>> {
        let mut heap = self.heap.lock_irqsave();
        let new_ptr = (*heap).reallocate_unknown_align(NonNull::new_unchecked(ptr), new_size);
        new_ptr
    }

    /// Retrieves various statistics about the current state of the heap's memory usage.
    ///
    /// # Arguments
    ///
    /// * `total` - Output parameter containing the total available memory on the heap.
    /// * `used` - Output parameter containing the currently used memory on the heap.
    /// * `max_used` - Output parameter containing the largest amount of memory ever used during execution.
    pub fn memory_info(&self) -> (usize, usize, usize) {
        let heap = self.heap.lock_irqsave();
        let x = ((*heap).total(), (*heap).allocated(), (*heap).maximum());
        x
    }
}
