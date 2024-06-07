use core::alloc::Layout;
use core::cell::RefCell;
use core::ptr::{self, NonNull};
use pinned_init::*;
use crate::allocator::{HeapLock, new_heap_lock};

pub mod tlsf_heap;
use tlsf_heap::Tlsf;

type TlsfHeap = Tlsf<'static, usize, usize, { usize::BITS as usize }, { usize::BITS as usize }>;

/// A two-Level segregated fit heap.
#[pin_data]
pub struct Heap {
    #[pin]
    heap: HeapLock<RefCell<TlsfHeap>>,
}

impl Heap {
    /// Create a new UNINITIALIZED heap allocator
    ///
    /// You must initialize this heap using the
    /// [`init`](Self::init) method before using the allocator.
    pub fn new() -> impl PinInit<Self> {
        pin_init!(Heap {
            heap <- new_heap_lock!(RefCell::new(ConstDefault::DEFAULT), "heap"),
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
        let block: &[u8] = core::slice::from_raw_parts(start_addr as *const u8, size);
        let mut heap = self.heap.lock();
        (*heap.get_mut()).insert_free_block_ptr(block.into());
    }

    pub fn alloc(&self, layout: Layout) -> Option<NonNull<u8>> {
        let mut heap = self.heap.lock();
        (*heap.get_mut()).allocate(&layout)
    }

    pub unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut heap = self.heap.lock();
        (*heap.get_mut()).deallocate(NonNull::new_unchecked(ptr), layout.align())
    }

    pub unsafe fn realloc(
        &self,
        ptr: *mut u8,
        layout: Layout,
        new_size: usize,
    ) -> Option<NonNull<u8>> {
        let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());
        let mut heap = self.heap.lock();
        (*heap.get_mut()).reallocate(NonNull::new_unchecked(ptr), &new_layout)
    }

    pub fn memory_info(&self) -> (usize, usize, usize) {
        let mut heap = self.heap.lock();
        (
            (*heap.get_mut()).total(),
            (*heap.get_mut()).used(),
            (*heap.get_mut()).maximum(),
        )
    }
}
