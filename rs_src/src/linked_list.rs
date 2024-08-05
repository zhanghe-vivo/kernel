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

unsafe impl Send for LinkedList {}

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
    pub unsafe fn push(&mut self, item: *mut usize) {
        *item = self.head as usize;
        self.head = item;
    }

    /// Try to remove the first item in the list
    pub fn pop(&mut self) -> Option<*mut usize> {
        match self.is_empty() {
            true => None,
            false => {
                // Advance head pointer
                let item = self.head;
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

impl<'a> Iterator for Iter<'a> {
    type Item = *mut usize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.curr.is_null() {
            None
        } else {
            let item = self.curr;
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
        // Skip the current one
        unsafe {
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

impl<'a> Iterator for IterMut<'a> {
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
            next: unsafe { Link::new_unchecked(this) },
            prev: unsafe { Link::new_unchecked(this) },
            pin: PhantomPinned,
        }? Infallible)
    }

    #[inline]
    pub fn insert_next(self: Pin<&mut Self>, list: &ListHead) {
        // SAFETY: We do not move `self`.
        let this: &mut Self = unsafe { self.get_unchecked_mut() };
        this.prev = list
            .next
            .prev()
            .replace(unsafe { Link::new_unchecked(NonNull::new_unchecked(this)) });
        this.next = list
            .next
            .replace(unsafe { Link::new_unchecked(NonNull::new_unchecked(this)) });
    }

    #[inline]
    pub fn insert_prev(self: Pin<&mut Self>, list: &ListHead) {
        // SAFETY: We do not move `self`.
        let this: &mut Self = unsafe { self.get_unchecked_mut() };
        this.next = list
            .prev
            .next()
            .replace(unsafe { Link::new_unchecked(NonNull::new_unchecked(this)) });
        this.prev = list
            .prev
            .replace(unsafe { Link::new_unchecked(NonNull::new_unchecked(this)) });
    }

    #[inline]
    pub fn reinit(self: Pin<&mut Self>) {
        let this: &mut Self = unsafe { self.get_unchecked_mut() };
        let ptr: *mut ListHead = this;
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
            let next = unsafe { &*self.next.as_ptr() };
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
}

#[pinned_drop]
impl PinnedDrop for ListHead {
    //#[inline]
    fn drop(self: Pin<&mut Self>) {
        if !ptr::eq(self.next.as_ptr(), &*self) {
            let next = unsafe { &*self.next.as_ptr() };
            let prev = unsafe { &*self.prev.as_ptr() };
            next.prev.set(&self.prev);
            prev.next.set(&self.next);
        }
    }
}

#[repr(transparent)]
#[derive(Clone, Debug)]
struct Link(Cell<NonNull<ListHead>>);

impl Link {
    #[inline]
    unsafe fn new_unchecked(ptr: NonNull<ListHead>) -> Self {
        Self(Cell::new(ptr))
    }

    #[inline]
    fn next(&self) -> &Link {
        unsafe { &(*self.0.get().as_ptr()).next }
    }

    #[inline]
    fn prev(&self) -> &Link {
        unsafe { &(*self.0.get().as_ptr()).prev }
    }

    #[allow(dead_code)]
    fn cur(&self) -> &ListHead {
        unsafe { &*self.0.get().as_ptr() }
    }

    #[inline]
    fn replace(&self, other: Link) -> Link {
        unsafe { Link::new_unchecked(self.0.replace(other.0.get())) }
    }

    #[inline]
    fn as_ptr(&self) -> *const ListHead {
        self.0.get().as_ptr()
    }

    #[inline]
    fn set(&self, val: &Link) {
        self.0.set(val.0.get());
    }
}

/// Get the struct for this entry
#[macro_export]
macro_rules! list_head_entry {
    ($node:expr, $type:ty, $($f:tt)*) => {
        crate::container_of!($node, $type, $($f)*)
    };
}

/// Iterate over a list
#[macro_export]
macro_rules! list_head_for_each {
    ($pos:ident, $head:expr, $code:block) => {
        let mut $pos = $head;
        while let Some(next) = $pos.next() {
            $pos = unsafe { &*next.as_ptr() };
            if core::ptr::eq($pos, $head) {
                break;
            }
            $code
        }
    };
}
