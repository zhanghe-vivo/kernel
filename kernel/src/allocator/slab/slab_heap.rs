use core::{alloc::Layout, ptr::NonNull};

use crate::allocator::{align_down_size, align_up_size, buddy::buddy_system_heap};
use blue_infra::list::doubly_linked_list::SinglyLinkedList;

pub struct Slab {
    block_size: usize,
    len: usize,
    free_block_list: SinglyLinkedList,
}

impl Slab {
    /// Create an empty heap
    pub const fn new() -> Self {
        Slab {
            block_size: 0,
            len: 0,
            free_block_list: SinglyLinkedList::new(),
        }
    }

    pub unsafe fn init(&mut self, start_addr: usize, slab_size: usize, block_size: usize) {
        let num_of_blocks = slab_size / block_size;
        self.block_size = block_size;
        for i in (0..num_of_blocks).rev() {
            let new_block = (start_addr + i * block_size) as *mut usize;
            self.free_block_list.push(new_block);
        }
        self.len = num_of_blocks;
    }

    pub fn allocate(&mut self, _layout: &Layout) -> Option<NonNull<u8>> {
        match self.free_block_list.pop() {
            Some(block) => {
                self.len -= 1;
                let ptr = unsafe { NonNull::new_unchecked(block as *mut u8) };
                Some(ptr)
            }
            None => None, //Err(AllocErr)
        }
    }

    /// Safety: ptr must have been previously allocated by self.
    pub unsafe fn deallocate(&mut self, ptr: NonNull<u8>) {
        // Since ptr was allocated by self, its alignment must be at least
        // the alignment of FreeBlock. Casting a less aligned pointer to
        // &mut FreeBlock would be undefined behavior.
        #[cfg_attr(feature = "cargo-clippy", allow(clippy::cast_ptr_alignment))]
        let ptr = ptr.as_ptr() as *mut usize;
        self.free_block_list.push(ptr);
        self.len += 1;
    }
}

#[derive(Copy, Clone)]
pub enum HeapAllocator {
    Slab64Bytes,
    Slab128Bytes,
    Slab256Bytes,
    Slab512Bytes,
    Slab1024Bytes,
    Slab2048Bytes,
    Slab4096Bytes,
    BuddySystemAllocator,
}

/// A fixed size heap backed by multiple slabs with blocks of different sizes.
/// Allocations over 4096 bytes are served by a buddy system allocator.
pub struct Heap {
    slab_64_bytes: Slab,
    slab_128_bytes: Slab,
    slab_256_bytes: Slab,
    slab_512_bytes: Slab,
    slab_1024_bytes: Slab,
    slab_2048_bytes: Slab,
    slab_4096_bytes: Slab,
    buddy_system_allocator: buddy_system_heap::Heap<32>,

    begin_addr: usize,
    slab_size: usize,
    // statistics
    allocated: usize,
    maximum: usize,
    total: usize,
}

impl Heap {
    /// Create an empty heap
    pub const fn new() -> Self {
        Heap {
            slab_64_bytes: Slab::new(),
            slab_128_bytes: Slab::new(),
            slab_256_bytes: Slab::new(),
            slab_512_bytes: Slab::new(),
            slab_1024_bytes: Slab::new(),
            slab_2048_bytes: Slab::new(),
            slab_4096_bytes: Slab::new(),
            buddy_system_allocator: buddy_system_heap::Heap::empty(),

            begin_addr: 0,
            slab_size: 0,
            allocated: 0,
            maximum: 0,
            total: 0,
        }
    }

    /// Create an empty heap
    pub const fn empty() -> Self {
        Self::new()
    }

    /// Add a range of memory [start, start+size) to the heap
    pub unsafe fn init(&mut self, start: usize, size: usize) {
        /* align begin and end addr to page */
        let begin_align = align_up_size(start, super::PAGE_SIZE);
        let end_align = align_down_size(start + size, super::PAGE_SIZE);
        let size = end_align - begin_align;

        self.begin_addr = begin_align;
        self.slab_size = size / super::NUM_OF_SLABS;
        self.slab_64_bytes.init(begin_align, self.slab_size, 64);
        self.slab_128_bytes
            .init(begin_align + self.slab_size, self.slab_size, 128);
        self.slab_256_bytes
            .init(begin_align + 2 * self.slab_size, self.slab_size, 256);
        self.slab_512_bytes
            .init(begin_align + 3 * self.slab_size, self.slab_size, 512);
        self.slab_1024_bytes
            .init(begin_align + 4 * self.slab_size, self.slab_size, 1024);
        self.slab_2048_bytes
            .init(begin_align + 5 * self.slab_size, self.slab_size, 2048);
        self.slab_4096_bytes
            .init(begin_align + 6 * self.slab_size, self.slab_size, 4096);
        self.buddy_system_allocator
            .init(begin_align + 7 * self.slab_size, self.slab_size);
        self.total += size;
    }
    /// Allocates a chunk of the given size with the given alignment. Returns a pointer to the
    /// beginning of that chunk if it was successful. Else it returns `()`.
    /// This function finds the slab of lowest size which can still accommodate the given chunk.
    /// The runtime is in `O(1)` for chunks of size <= 4096, and `probably fast` when chunk size is > 4096,
    pub fn allocate(&mut self, layout: &Layout) -> Option<NonNull<u8>> {
        let ptr;
        match Heap::layout_to_allocator(layout.size(), layout.align()) {
            HeapAllocator::Slab64Bytes => {
                ptr = self.slab_64_bytes.allocate(layout)?;
                self.allocated += 64;
            }
            HeapAllocator::Slab128Bytes => {
                ptr = self.slab_128_bytes.allocate(layout)?;
                self.allocated += 128;
            }
            HeapAllocator::Slab256Bytes => {
                ptr = self.slab_256_bytes.allocate(layout)?;
                self.allocated += 256;
            }
            HeapAllocator::Slab512Bytes => {
                ptr = self.slab_512_bytes.allocate(layout)?;
                self.allocated += 512;
            }
            HeapAllocator::Slab1024Bytes => {
                ptr = self.slab_1024_bytes.allocate(layout)?;
                self.allocated += 1024;
            }
            HeapAllocator::Slab2048Bytes => {
                ptr = self.slab_2048_bytes.allocate(layout)?;
                self.allocated += 2048;
            }
            HeapAllocator::Slab4096Bytes => {
                ptr = self.slab_4096_bytes.allocate(layout)?;
                self.allocated += 4096;
            }
            HeapAllocator::BuddySystemAllocator => {
                ptr = self.buddy_system_allocator.allocate(layout)?;
                self.allocated += layout.size();
            }
        }
        self.maximum = core::cmp::max(self.maximum, self.allocated);
        Some(ptr)
    }

    /// Frees the given allocation. `ptr` must be a pointer returned
    /// by a call to the `allocate` function with identical size and alignment.
    ///
    /// This function finds the slab which contains address of `ptr` and adds the blocks beginning
    /// with `ptr` address to the list of free blocks.
    /// This operation is in `O(1)` for blocks <= 4096 bytes and `probably fast` for blocks > 4096 bytes.
    ///
    /// # Safety
    ///
    /// Undefined behavior may occur for invalid arguments, thus this function is unsafe.
    pub unsafe fn deallocate(&mut self, ptr: NonNull<u8>, layout: &Layout) {
        let mut size = layout.size();
        if size == 0 {
            // from c
            let slab_index = (ptr.as_ptr() as usize - self.begin_addr) / self.slab_size;
            size = 64 * (slab_index + 1);
        }

        match Heap::layout_to_allocator(size, layout.align()) {
            HeapAllocator::Slab64Bytes => {
                self.slab_64_bytes.deallocate(ptr);
                self.allocated -= 64;
            }
            HeapAllocator::Slab128Bytes => {
                self.slab_128_bytes.deallocate(ptr);
                self.allocated -= 128;
            }
            HeapAllocator::Slab256Bytes => {
                self.slab_256_bytes.deallocate(ptr);
                self.allocated -= 256;
            }
            HeapAllocator::Slab512Bytes => {
                self.slab_512_bytes.deallocate(ptr);
                self.allocated -= 512;
            }
            HeapAllocator::Slab1024Bytes => {
                self.slab_1024_bytes.deallocate(ptr);
                self.allocated -= 1024;
            }
            HeapAllocator::Slab2048Bytes => {
                self.slab_2048_bytes.deallocate(ptr);
                self.allocated -= 2048;
            }
            HeapAllocator::Slab4096Bytes => {
                self.slab_4096_bytes.deallocate(ptr);
                self.allocated -= 4096;
            }
            HeapAllocator::BuddySystemAllocator => {
                let size = self
                    .buddy_system_allocator
                    .get_block_size(ptr, layout.align());
                self.buddy_system_allocator.deallocate(ptr, layout);
                self.allocated -= size;
            }
        }
    }

    pub unsafe fn reallocate(
        &mut self,
        ptr: NonNull<u8>,
        new_layout: &Layout,
    ) -> Option<NonNull<u8>> {
        let slab_index = (ptr.as_ptr() as usize - self.begin_addr) / self.slab_size;
        let size = 64 * (slab_index + 1);
        if size <= 4096 {
            // can use old block
            if new_layout.size() <= size {
                return Some(ptr);
            }

            // Allocate a whole new memory block
            let new_ptr = self.allocate(new_layout)?;
            // Move the existing data into the new location
            core::ptr::copy_nonoverlapping(ptr.as_ptr(), new_ptr.as_ptr(), size);
            // Deallocate the old memory block.
            let layout = Layout::from_size_align(size, new_layout.align()).unwrap();
            self.deallocate(ptr, &layout);

            return Some(new_ptr);
        } else {
            return self.buddy_system_allocator.reallocate(ptr, new_layout);
        }
    }

    ///Finds allocator to use based on layout size and alignment
    pub fn layout_to_allocator(size: usize, align: usize) -> HeapAllocator {
        if size > 4096 {
            HeapAllocator::BuddySystemAllocator
        } else if size <= 64 && align <= 64 {
            HeapAllocator::Slab64Bytes
        } else if size <= 128 && align <= 128 {
            HeapAllocator::Slab128Bytes
        } else if size <= 256 && align <= 256 {
            HeapAllocator::Slab256Bytes
        } else if size <= 512 && align <= 512 {
            HeapAllocator::Slab512Bytes
        } else if size <= 1024 && align <= 1024 {
            HeapAllocator::Slab1024Bytes
        } else if size <= 2048 && align <= 2048 {
            HeapAllocator::Slab2048Bytes
        } else {
            HeapAllocator::Slab4096Bytes
        }
    }

    /// Return the number of bytes that maximum used
    pub fn maximum(&self) -> usize {
        self.maximum
    }

    /// Return the number of bytes that are actually allocated
    pub fn allocated(&self) -> usize {
        self.allocated
    }

    /// Return the total number of bytes in the heap
    pub fn total(&self) -> usize {
        self.total
    }
}
