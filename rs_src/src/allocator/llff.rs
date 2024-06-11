use crate::sync::{new_heaplock, HeapLock};
use core::alloc::Layout;
use core::cell::RefCell;
use core::ptr::NonNull;
use pinned_init::*;

pub mod linked_list_heap;
use linked_list_heap::Heap as LLHeap;

/// A linked list first fit heap.
#[pin_data]
pub struct Heap {
    #[pin]
    heap: HeapLock<RefCell<LLHeap>>,
}

impl Heap {
    /// Create a new UNINITIALIZED heap allocator
    ///
    /// You must initialize this heap using the
    /// [`init`](Self::init) method before using the allocator.
    pub fn new() -> impl PinInit<Self> {
        pin_init!(Heap {
            heap <- new_heaplock!(RefCell::new(LLHeap::empty()), "heap"),
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
        let heap = self.heap.lock();
        (*heap).borrow_mut().init(start_addr, size);
    }

    pub fn alloc(&self, layout: Layout) -> Option<NonNull<u8>> {
        let heap = self.heap.lock();
        let ptr = (*heap).borrow_mut().allocate_first_fit(&layout);
        ptr
    }

    pub unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let heap = self.heap.lock();
        (*heap)
            .borrow_mut()
            .deallocate(NonNull::new_unchecked(ptr), &layout);
    }

    pub unsafe fn realloc(
        &self,
        ptr: *mut u8,
        layout: Layout,
        new_size: usize,
    ) -> Option<NonNull<u8>> {
        let heap = self.heap.lock();
        let new_ptr = (*heap)
            .borrow_mut()
            .realloc(NonNull::new_unchecked(ptr), &layout, new_size);
        new_ptr
    }

    pub fn memory_info(&self) -> (usize, usize, usize) {
        let heap = self.heap.lock();
        let x = (
            (*heap).borrow_mut().total(),
            (*heap).borrow_mut().allocated(),
            (*heap).borrow_mut().maximum(),
        );
        x
    }
}
