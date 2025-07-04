use super::Heap;
use crate::{allocator::MemoryInfo, sync::SpinLock};
use core::{alloc::Layout, ptr::NonNull};

pub struct LlffHeap {
    heap: SpinLock<Heap>,
}

impl LlffHeap {
    pub const fn new() -> Self {
        Self {
            heap: SpinLock::new(Heap::empty()),
        }
    }

    pub unsafe fn init(&self, start_addr: usize, size: usize) {
        let mut heap = self.heap.irqsave_lock();
        (*heap).init(start_addr, size);
    }

    pub fn alloc(&self, layout: Layout) -> Option<NonNull<u8>> {
        let mut heap = self.heap.irqsave_lock();
        let ptr = (*heap).allocate_first_fit(&layout);
        ptr
    }

    pub unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut heap = self.heap.irqsave_lock();
        (*heap).deallocate(NonNull::new_unchecked(ptr), &layout);
    }

    pub unsafe fn deallocate_unknown_align(&self, ptr: *mut u8) {
        let mut heap = self.heap.irqsave_lock();
        (*heap).deallocate_unknown_align(NonNull::new_unchecked(ptr));
    }

    pub unsafe fn realloc(
        &self,
        ptr: *mut u8,
        layout: Layout,
        new_size: usize,
    ) -> Option<NonNull<u8>> {
        let mut heap = self.heap.irqsave_lock();
        let new_ptr = (*heap).realloc(NonNull::new_unchecked(ptr), &layout, new_size);
        new_ptr
    }

    pub unsafe fn realloc_unknown_align(
        &self,
        ptr: *mut u8,
        new_size: usize,
    ) -> Option<NonNull<u8>> {
        let mut heap = self.heap.irqsave_lock();
        let new_ptr = (*heap).realloc_unknown_align(NonNull::new_unchecked(ptr), new_size);
        new_ptr
    }

    pub fn memory_info(&self) -> MemoryInfo {
        let heap = self.heap.irqsave_lock();
        MemoryInfo {
            total: (*heap).total(),
            used: (*heap).allocated(),
            max_used: (*heap).maximum(),
        }
    }
}
