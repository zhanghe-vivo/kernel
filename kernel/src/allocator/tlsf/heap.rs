use super::Tlsf;
use crate::{allocator, sync::spinlock::SpinLock};
use const_default::ConstDefault;
use core::{alloc::Layout, ptr::NonNull};

use allocator::MemoryInfo;

type TlsfHeap = Tlsf<'static, usize, usize, { usize::BITS as usize }, { usize::BITS as usize }>;

/// A two-Level segregated fit heap.
pub struct Heap {
    heap: SpinLock<TlsfHeap>,
}

impl Heap {
    // Create a new UNINITIALIZED heap allocator
    pub const fn new() -> Self {
        Heap {
            heap: SpinLock::new(ConstDefault::DEFAULT),
        }
    }

    // Initializes the heap
    pub unsafe fn init(&self, start_addr: usize, size: usize) {
        let block: &[u8] = core::slice::from_raw_parts(start_addr as *const u8, size);
        let mut heap = self.heap.irqsave_lock();
        heap.insert_free_block_ptr(block.into());
    }

    // try to allocate memory with the given layout
    pub fn alloc(&self, layout: Layout) -> Option<NonNull<u8>> {
        let mut heap = self.heap.irqsave_lock();
        let ptr = heap.allocate(&layout);
        ptr
    }

    // deallocate the memory pointed by ptr with the given layout
    pub unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut heap = self.heap.irqsave_lock();
        heap.deallocate(NonNull::new_unchecked(ptr), layout.align());
    }

    pub unsafe fn deallocate_unknown_align(&self, ptr: *mut u8) {
        let mut heap = self.heap.irqsave_lock();
        heap.deallocate_unknown_align(NonNull::new_unchecked(ptr));
    }

    // reallocate memory with the given size and layout
    pub unsafe fn realloc(
        &self,
        ptr: *mut u8,
        layout: Layout,
        new_size: usize,
    ) -> Option<NonNull<u8>> {
        let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());
        let mut heap = self.heap.irqsave_lock();
        let new_ptr = heap.reallocate(NonNull::new_unchecked(ptr), &new_layout);
        new_ptr
    }

    // reallocate memory with the given size but with out align
    pub unsafe fn realloc_unknown_align(
        &self,
        ptr: *mut u8,
        new_size: usize,
    ) -> Option<NonNull<u8>> {
        let mut heap = self.heap.irqsave_lock();
        let new_ptr = heap.reallocate_unknown_align(NonNull::new_unchecked(ptr), new_size);
        new_ptr
    }

    // Retrieves various statistics about the current state of the heap's memory usage.
    pub fn memory_info(&self) -> MemoryInfo {
        let heap = self.heap.irqsave_lock();
        MemoryInfo {
            total: heap.total(),
            used: heap.allocated(),
            max_used: heap.maximum(),
        }
    }
}
