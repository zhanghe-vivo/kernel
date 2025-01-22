#![allow(dead_code)]
use core::{alloc::Layout, cmp, fmt, mem, ptr::NonNull};

use crate::allocator::block_hdr::*;
use blue_infra::list::doubly_linked_list::SinglyLinkedList;
/// A heap that uses buddy system with configurable order.
///
/// # Usage
///
/// Create a heap and add a memory region to it:
/// ```
/// use buddy_system_allocator::*;
/// # use core::mem::size_of;
/// let mut heap = Heap::<32>::empty();
/// # let space: [usize; 100] = [0; 100];
/// # let begin: usize = space.as_ptr() as usize;
/// # let end: usize = begin + 100 * size_of::<usize>();
/// # let size: usize = 100 * size_of::<usize>();
/// unsafe {
///     heap.init(begin, size);
///     // or
///     heap.add_to_heap(begin, end);
/// }
/// ```
pub struct Heap<const ORDER: usize> {
    // buddy system with max order of `ORDER`
    free_list: [SinglyLinkedList; ORDER],

    // statistics
    allocated: usize,
    maximum: usize,
    total: usize,
}

impl<const ORDER: usize> Heap<ORDER> {
    /// Create an empty heap
    pub const fn new() -> Self {
        Heap {
            free_list: [SinglyLinkedList::new(); ORDER],
            maximum: 0,
            allocated: 0,
            total: 0,
        }
    }

    /// Create an empty heap
    pub const fn empty() -> Self {
        Self::new()
    }

    /// Add a range of memory [start, end) to the heap
    pub unsafe fn add_to_heap(&mut self, mut start: usize, mut end: usize) {
        start = start.wrapping_add(GRANULARITY - 1) & !(GRANULARITY - 1);
        end &= !(GRANULARITY - 1);
        assert!(start <= end);

        let mut total = 0;
        let mut current_start = start;

        while current_start + mem::size_of::<usize>() <= end {
            let lowbit = current_start & (!current_start + 1);
            let size = cmp::min(lowbit, prev_power_of_two(end - current_start));
            total += size;

            self.free_list[size.trailing_zeros() as usize].push(current_start as *mut usize);
            current_start += size;
        }

        self.total += total;
    }

    /// Add a range of memory [start, start+size) to the heap
    pub unsafe fn init(&mut self, start: usize, size: usize) {
        self.add_to_heap(start, start + size);
    }

    /// Alloc a range of memory from the heap satifying `layout` requirements
    pub fn allocate(&mut self, layout: &Layout) -> Option<NonNull<u8>> {
        let (max_overhead, search_size) = get_overhead_and_size(layout)?;
        let search_size = search_size.next_power_of_two();
        let class = search_size.trailing_zeros() as usize;

        for i in class..self.free_list.len() {
            // Find the first non-empty size class
            if !self.free_list[i].is_empty() {
                // Split buffers
                for j in (class + 1..i + 1).rev() {
                    if let Some(block) = self.free_list[j].pop() {
                        unsafe {
                            self.free_list[j - 1]
                                .push((block as usize + (1 << (j - 1))) as *mut usize);
                            self.free_list[j - 1].push(block);
                        }
                    } else {
                        return None;
                    }
                }

                let result = NonNull::new(
                    self.free_list[class]
                        .pop()
                        .expect("current block should have free space now")
                        as *mut u8,
                );
                if let Some(result) = result {
                    unsafe {
                        // Decide the starting address of the payload
                        let unaligned_ptr =
                            result.as_ptr() as *mut u8 as usize + mem::size_of::<UsedBlockHdr>();
                        let ptr = NonNull::new_unchecked(
                            (unaligned_ptr.wrapping_add(layout.align() - 1) & !(layout.align() - 1))
                                as *mut u8,
                        );

                        if layout.align() < GRANULARITY {
                            debug_assert_eq!(unaligned_ptr, ptr.as_ptr() as usize);
                        } else {
                            debug_assert_ne!(unaligned_ptr, ptr.as_ptr() as usize);
                        }

                        // Calculate the actual overhead and the final block size of the
                        // used block being created here
                        debug_assert!(
                            (ptr.as_ptr() as usize - result.as_ptr() as usize) <= max_overhead
                        );

                        // Turn `block` into a used memory block and initialize the used block
                        // header. `prev_phys_block` is already set.
                        let mut block = result.cast::<UsedBlockHdr>();
                        block.as_mut().common.size = search_size | SIZE_USED;

                        // Place a `UsedBlockPad` (used by `used_block_hdr_for_allocation`)
                        if layout.align() >= GRANULARITY {
                            (*UsedBlockPad::get_for_allocation(ptr)).block_hdr = block;
                        }

                        self.allocated += search_size;
                        self.maximum = cmp::max(self.maximum, self.allocated);
                        return Some(ptr);
                    }
                } else {
                    return None;
                }
            }
        }
        None
    }

    pub unsafe fn get_block_size(&mut self, ptr: NonNull<u8>, align: usize) -> usize {
        let old_block = used_block_hdr_for_allocation(ptr, align).cast::<BlockHdr>();
        old_block.as_ref().size & !SIZE_USED
    }

    /// Dealloc a range of memory from the heap
    pub unsafe fn deallocate(&mut self, ptr: NonNull<u8>, layout: &Layout) {
        // Safety: `ptr` is a previously allocated memory block with the same
        //         alignment as `align`. This is upheld by the caller.
        let old_block = used_block_hdr_for_allocation(ptr, layout.align()).cast::<BlockHdr>();
        let size = old_block.as_ref().size & !SIZE_USED;
        let class = size.trailing_zeros() as usize;

        unsafe {
            // Put back into free list
            self.free_list[class].push(old_block.as_ptr() as *mut usize);

            // Merge free buddy lists
            let mut current_ptr = old_block.as_ptr() as usize;
            let mut current_class = class;

            while current_class < self.free_list.len() - 1 {
                let buddy = current_ptr ^ (1 << current_class);
                let mut flag = false;
                for block in self.free_list[current_class].iter_mut() {
                    if block.value() as usize == buddy {
                        block.pop();
                        flag = true;
                        break;
                    }
                }

                // Free buddy found
                if flag {
                    self.free_list[current_class].pop();
                    current_ptr = cmp::min(current_ptr, buddy);
                    current_class += 1;
                    self.free_list[current_class].push(current_ptr as *mut usize);
                } else {
                    break;
                }
            }
        }

        self.allocated -= size;
    }

    pub unsafe fn reallocate(
        &mut self,
        ptr: NonNull<u8>,
        new_layout: &Layout,
    ) -> Option<NonNull<u8>> {
        // Safety: `ptr` is a previously allocated memory block with the same
        //         alignment as `align`. This is upheld by the caller.
        let block = used_block_hdr_for_allocation(ptr, new_layout.align());
        let overhead = ptr.as_ptr() as usize - block.as_ptr() as usize;
        let new_size = overhead.checked_add(new_layout.size())?;
        let new_size = new_size.checked_add(GRANULARITY - 1)? & !(GRANULARITY - 1);
        let old_size = block.as_ref().common.size - SIZE_USED;

        if new_size <= old_size {
            return Some(ptr);
        }

        let old_size = old_size - overhead;

        // Allocate a whole new memory block
        let new_ptr = self.allocate(new_layout)?;

        // Move the existing data into the new location
        debug_assert!(new_layout.size() >= old_size);
        core::ptr::copy_nonoverlapping(ptr.as_ptr(), new_ptr.as_ptr(), old_size);

        // Deallocate the old memory block.
        self.deallocate(ptr, new_layout);

        Some(new_ptr)
    }

    /// Return the number of bytes that maximum used
    pub fn stats_alloc_max(&self) -> usize {
        self.maximum
    }

    /// Return the number of bytes that are actually allocated
    pub fn stats_alloc_actual(&self) -> usize {
        self.allocated
    }

    /// Return the total number of bytes in the heap
    pub fn stats_total_bytes(&self) -> usize {
        self.total
    }
}

impl<const ORDER: usize> fmt::Debug for Heap<ORDER> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Heap")
            .field("maximum", &self.maximum)
            .field("allocated", &self.allocated)
            .field("total", &self.total)
            .finish()
    }
}

pub(crate) fn prev_power_of_two(num: usize) -> usize {
    1 << (usize::BITS as usize - num.leading_zeros() as usize - 1)
}
