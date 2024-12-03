//! Provide the intrusive LinkedList and ListHead
#![allow(dead_code)]
use core::{
    cell::Cell,
    convert::Infallible,
    fmt,
    marker::{PhantomData, PhantomPinned},
    pin::Pin,
    ptr::{self, NonNull},
};
use pinned_init::*;

/// An intrusive linked list
///
/// A clean room implementation of the one used in CS140e 2018 Winter
///
/// Thanks Sergio Benitez for his excellent work,
/// See [CS140e](https://cs140e.sergio.bz/) for more information
#[derive(Copy, Clone)]
pub struct LinkedList {
    head: *mut usize,
}

/// SAFETY: LinkedList only contains a raw pointer field, which is safe to share between threads
/// because all modification operations require exclusive mutable reference (&mut self),
/// ensuring that only one thread can modify the list at any time.
unsafe impl Send for LinkedList {}

impl Default for LinkedList {
    fn default() -> Self {
        Self::new()
    }
}

impl LinkedList {
    /// Create a new LinkedList
    pub const fn new() -> LinkedList {
        LinkedList {
            head: ptr::null_mut(),
        }
    }

    /// Return `true` if the list is empty
    pub fn is_empty(&self) -> bool {
        self.head.is_null()
    }

    /// Push `item` to the front of the list
    /// # SAFETY
    /// - `item` must be a valid, non-null pointer
    /// - `item` must point to valid memory that can be written to
    pub unsafe fn push(&mut self, item: *mut usize) {
        debug_assert!(!item.is_null(), "item cannot be null");

        *item = self.head as usize;
        self.head = item;
    }

    /// Try to remove the first item in the list
    pub fn pop(&mut self) -> Option<*mut usize> {
        match self.is_empty() {
            true => None,
            false => {
                let item = self.head;
                // SAFETY: We have checked that self.head is not null, and item points to 
                // a valid address stored by previous push operations
                self.head = unsafe { *item as *mut usize };
                Some(item)
            }
        }
    }

    /// Return an iterator over the items in the list
    pub fn iter(&self) -> Iter {
        Iter {
            curr: self.head,
            list: PhantomData,
        }
    }

    /// Return an mutable iterator over the items in the list
    pub fn iter_mut(&mut self) -> IterMut {
        IterMut {
            prev: &mut self.head as *mut *mut usize as *mut usize,
            curr: self.head,
            list: PhantomData,
        }
    }
}

impl fmt::Debug for LinkedList {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

/// An iterator over the linked list
pub struct Iter<'a> {
    curr: *mut usize,
    list: PhantomData<&'a LinkedList>,
}

impl Iterator for Iter<'_> {
    type Item = *mut usize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.curr.is_null() {
            None
        } else {
            let item = self.curr;
            // SAFETY: We have checked that self.curr is not null
            let next = unsafe { *item as *mut usize };
            self.curr = next;
            Some(item)
        }
    }
}

/// Represent a mutable node in `LinkedList`
pub struct ListNode {
    prev: *mut usize,
    curr: *mut usize,
}

impl ListNode {
    /// Remove the node from the list
    pub fn pop(self) -> *mut usize {
        debug_assert!(!self.prev.is_null(), "prev pointer cannot be null");
        debug_assert!(!self.curr.is_null(), "curr pointer cannot be null");

         // SAFETY: We have checked that self.prev and self.curr are not null
        // If prev or curr is null, the list is corrupted
        unsafe {
            // skip the curr one.
            *(self.prev) = *(self.curr);
        }
        self.curr
    }

    /// Returns the pointed address
    pub fn value(&self) -> *mut usize {
        self.curr
    }
}

/// A mutable iterator over the linked list
pub struct IterMut<'a> {
    list: PhantomData<&'a mut LinkedList>,
    prev: *mut usize,
    curr: *mut usize,
}

impl Iterator for IterMut<'_> {
    type Item = ListNode;

    fn next(&mut self) -> Option<Self::Item> {
        if self.curr.is_null() {
            None
        } else {
            let res = ListNode {
                prev: self.prev,
                curr: self.curr,
            };
            self.prev = self.curr;
            // SAFETY: We have checked that self.curr is not null
            self.curr = unsafe { *self.curr as *mut usize };
            Some(res)
        }
    }
}

#[pin_data(PinnedDrop)]
#[repr(C)]
#[derive(Debug)]
pub struct ListHead {
    pub(crate) next: Link,
    pub(crate) prev: Link,
    #[pin]
    pin: PhantomPinned,
}

impl ListHead {
    #[inline]
    pub fn new() -> impl PinInit<Self, Infallible> {
        try_pin_init!(&this in Self {
            // SAFETY: We ensure that this is a valid pointer
            next: unsafe { Link::new_unchecked(this) },
            // SAFETY: We ensure that this is a valid pointer
            prev: unsafe { Link::new_unchecked(this) },
            pin: PhantomPinned,
        }? Infallible)
    }

    #[inline]
    pub fn insert_next(self: Pin<&mut Self>, list: &ListHead) {
        // SAFETY: We do not move `self`.
        let this: &mut Self = unsafe { self.get_unchecked_mut() };
        // SAFETY: We ensure that this is a valid pointer
        unsafe {
        this.prev = list
            .next
            .prev()
            .replace(Link::new_unchecked(NonNull::new_unchecked(this)));
        this.next = list
            .next
            .replace(Link::new_unchecked(NonNull::new_unchecked(this)));
        }
    }

    #[inline]
    pub fn insert_prev(self: Pin<&mut Self>, list: &ListHead) {
        // SAFETY: We do not move `self`.
        let this: &mut Self = unsafe { self.get_unchecked_mut() };
        // SAFETY: We ensure that this is a valid pointer
        unsafe {
        this.next = list
            .prev
            .next()
            .replace( Link::new_unchecked(NonNull::new_unchecked(this)));
        this.prev = list
            .prev
            .replace(Link::new_unchecked(NonNull::new_unchecked(this)));
        }
    }

    #[inline]
    pub fn reinit(self: Pin<&mut Self>) {
        // SAFETY: We do not move `self`
        let this: &mut Self = unsafe { self.get_unchecked_mut() };
        let ptr: *mut ListHead = this;
        // SAFETY: We ensure that this is a valid pointer
        unsafe {
            (*ptr)
                .prev
                .replace(Link::new_unchecked(NonNull::new_unchecked(ptr)));
            (*ptr)
                .next
                .replace(Link::new_unchecked(NonNull::new_unchecked(ptr)));
        }
    }

    #[inline]
    pub fn remove(self: Pin<&mut Self>) {
        if !ptr::eq(self.next.as_ptr(), &*self) {
            debug_assert!(!self.next.as_ptr().is_null(), "next pointer cannot be null");
            debug_assert!(!self.prev.as_ptr().is_null(), "prev pointer cannot be null");

            // SAFETY: We ensure that self.next and self.prev are valid pointers
            // If next or prev is null, the list is corrupted
            let next = unsafe { &*self.next.as_ptr() };
            // SAFETY: self.prev.as_ptr() is a valid pointer
            let prev = unsafe { &*self.prev.as_ptr() };
            next.prev.set(&self.prev);
            prev.next.set(&self.next);
            // set self as empty.
            self.reinit();
        }
    }

    #[inline]
    pub fn next(&self) -> Option<NonNull<Self>> {
        if ptr::eq(self.next.as_ptr(), self) {
            None
        } else {
            // SAFETY: Insertion ensures that self.next is a valid pointer
            // If next is null, the list is corrupted
            Some(unsafe { NonNull::new_unchecked(self.next.as_ptr() as *mut Self) })
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        ptr::eq(self.next.as_ptr(), self)
    }

    #[allow(dead_code)]
    pub fn size(&self) -> usize {
        let mut size = 1;
        let mut cur = self.next.clone();
        while !ptr::eq(self, cur.cur()) {
            cur = cur.next().clone();
            size += 1;
        }
        size
    }

    #[inline]
    pub fn as_ptr(&self) -> *const ListHead {
        self as *const ListHead
    }
}

#[pinned_drop]
impl PinnedDrop for ListHead {
    //#[inline]
    fn drop(self: Pin<&mut Self>) {
        if !ptr::eq(self.next.as_ptr(), &*self) {
            // SAFETY: We ensure that self.next and self.prev are valid pointers
            // If next or prev is null, the list is corrupted
            let next = unsafe { &*self.next.as_ptr() };
            // SAFETY: self.prev.as_ptr() is a valid pointer
            let prev = unsafe { &*self.prev.as_ptr() };
            next.prev.set(&self.prev);
            prev.next.set(&self.next);
        }
    }
}

#[repr(transparent)]
#[derive(Clone, Debug)]
pub struct Link(Cell<NonNull<ListHead>>);

impl Link {
    // SAFETY: user must ensure that ptr is a valid pointer
    #[inline]
    unsafe fn new_unchecked(ptr: NonNull<ListHead>) -> Self {
        Self(Cell::new(ptr))
    }

    #[inline]
    fn next(&self) -> &Link {
        // SAFETY: self.0.get() is a NonNull<ListHead>, so self.0.get().as_ptr() is a valid pointer
        unsafe { &(*self.0.get().as_ptr()).next }
    }

    #[inline]
    fn prev(&self) -> &Link {
        // SAFETY: self.0.get() is a NonNull<ListHead>, so self.0.get().as_ptr() is a valid pointer
        unsafe { &(*self.0.get().as_ptr()).prev }
    }

    #[allow(dead_code)]
    fn cur(&self) -> &ListHead {
        // SAFETY: self.0.get() is a NonNull<ListHead>, so self.0.get().as_ptr() is a valid pointer
        unsafe { &*self.0.get().as_ptr() }
    }

    #[inline]
    fn replace(&self, other: Link) -> Link {
        // SAFETY: other.0.get() is a NonNull<ListHead>, so other.0.get().as_ptr() is a valid pointer
        unsafe { Link::new_unchecked(self.0.replace(other.0.get())) }
    }

    #[inline]
    pub fn as_ptr(&self) -> *const ListHead {
        self.0.get().as_ptr()
    }

    #[inline]
    fn set(&self, val: &Link) {
        self.0.set(val.0.get());
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;
    extern crate test;

    use alloc::boxed::Box;

    use super::*;
    use test::{black_box, Bencher};

    #[bench]
    fn bench_push(b: &mut Bencher) {
        let n = 1 << 16;
        let mut l = LinkedList::new();
        b.iter(|| unsafe {
            for _ in 0..n {
                let v = Box::new(0usize);
                // Let the memory leak.
                black_box(l.push(Box::<usize>::into_raw(v)))
            }
        });
    }

    #[bench]
    fn bench_push_and_pop(b: &mut Bencher) {
        let n = 1 << 16;
        let mut l = LinkedList::new();
        b.iter(|| unsafe {
            for _ in 0..n {
                let v = Box::new(0usize);
                // Let the memory leak.
                black_box(l.push(Box::<usize>::into_raw(v)))
            }
            while !l.is_empty() {
                l.pop();
            }
        });
    }
}
