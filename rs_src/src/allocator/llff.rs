use core::alloc::Layout;
use core::cell::RefCell;
use core::ptr::{self, NonNull};
use pinned_init::*;
use crate::allocator::{HeapLock, new_heap_lock};

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
            heap <- new_heap_lock!(RefCell::new(LLHeap::empty()), "heap"),
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
        (*heap.get_mut()).init(start_addr as *mut u8, size);
    }

    pub fn alloc(&self, layout: Layout) -> Option<NonNull<u8>> {
        let mut heap = self.heap.lock();
        (*heap.get_mut()).allocate_first_fit(&layout)
    }

    pub unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut heap = self.heap.lock();
        (*heap.get_mut()).deallocate(NonNull::new_unchecked(ptr), &layout);
    }

    pub unsafe fn realloc(
        &self,
        ptr: *mut u8,
        layout: Layout,
        new_size: usize,
    ) -> Option<NonNull<u8>> {
        let mut heap = self.heap.lock();
        (*heap.get_mut()).realloc(NonNull::new_unchecked(ptr), &layout, new_size)
    }

    pub fn memory_info(&self) -> (usize, usize, usize) {
        let mut heap = self.heap.lock();
        (
            (*heap.get_mut()).size(),
            (*heap.get_mut()).used(),
            (*heap.get_mut()).maximum(),
        )
    }
}
