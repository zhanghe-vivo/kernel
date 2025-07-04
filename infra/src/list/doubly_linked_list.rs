// Copyright (c) 2025 vivo Mobile Communication Co., Ltd.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//       http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Provides an intrusive doubly linked list implementation
//!
//! This module provides two main list implementations:
//! 1. `LinkedListNode`/`ListHead` - A basic intrusive doubly linked list node implementation
//! 2. `LinkedList<T>` - A safe wrapper around `ListHead`, providing functionality similar to the standard library's LinkedList
//!
//! # Design Features
//! - Uses `Pin` to ensure nodes don't move in memory
//! - Implements interior mutability using `Cell`
//! - Supports zero-cost abstraction with intrusive design
//! - Provides complete iterator support

#![allow(dead_code)]
extern crate alloc;

use alloc::boxed::Box;
use core::{
    cell::Cell,
    marker::{PhantomData, PhantomPinned},
    ops::{Deref, DerefMut},
    pin::Pin,
    ptr::{self, NonNull},
};
use pinned_init::{pin_data, pin_init, pinned_drop, InPlaceInit, PinInit};

/// A doubly linked list node structure
///
/// This is a pinned structure that ensures it won't move in memory.
/// Maintains list connectivity through `next` and `prev` fields.
#[pin_data(PinnedDrop)]
#[repr(C)]
#[derive(Debug)]
pub struct LinkedListNode {
    pub(crate) next: Link,
    pub(crate) prev: Link,
    #[pin]
    pin: PhantomPinned,
}

/// `ListHead` is a type alias for `LinkedListNode`, used as the list head
pub type ListHead = LinkedListNode;

impl LinkedListNode {
    /// Creates a new list node
    ///
    /// Initially, the node's next and prev point to itself, forming a single-node ring
    #[inline]
    pub fn new() -> impl PinInit<Self> {
        pin_init!(&this in Self {
            // SAFETY: We ensure that this is a valid pointer
            next: unsafe { Link::new_unchecked(this) },
            // SAFETY: We ensure that this is a valid pointer
            prev: unsafe { Link::new_unchecked(this) },
            pin: PhantomPinned,
        })
    }

    /// Inserts the current node after the specified node
    ///
    /// # Safety
    /// - The current node must be unlinked (is_empty() == true)
    #[inline]
    pub fn insert_after(self: Pin<&mut Self>, list: Pin<&mut LinkedListNode>) {
        debug_assert!(self.is_empty(), "self is not empty!");

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

    /// Inserts the current node before the specified node
    ///
    /// # Safety
    /// - The current node must be unlinked (is_empty() == true)
    #[inline]
    pub fn insert_before(self: Pin<&mut Self>, list: Pin<&mut LinkedListNode>) {
        debug_assert!(self.is_empty(), "self is not empty!");

        // SAFETY: We do not move `self`.
        let this: &mut Self = unsafe { self.get_unchecked_mut() };
        // SAFETY: We ensure that this is a valid pointer
        unsafe {
            this.next = list
                .prev
                .next()
                .replace(Link::new_unchecked(NonNull::new_unchecked(this)));
            this.prev = list
                .prev
                .replace(Link::new_unchecked(NonNull::new_unchecked(this)));
        }
    }

    /// Resets the node state, making it a standalone ring
    #[inline]
    pub fn reset(self: Pin<&mut Self>) {
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

    /// Removes the current node from the list
    ///
    /// If the node is already standalone (is_empty() == true), no operation is performed
    #[inline]
    pub fn remove_from_list(self: Pin<&mut Self>) {
        if !self.is_empty() {
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
            self.reset();
        }
    }

    /// Gets a reference to the next node
    ///
    /// Returns None if the current node is standalone or is the last node in the list
    #[inline]
    pub fn next(&self) -> Option<NonNull<LinkedListNode>> {
        if self.is_empty() {
            None
        } else {
            // SAFETY: Insertion ensures that self.next is a valid pointer
            // If next is null, the list is corrupted
            Some(unsafe { NonNull::new_unchecked(self.next.as_ptr() as *mut Self) })
        }
    }

    /// Gets a reference to the previous node
    ///
    /// Returns None if the current node is standalone or is the first node in the list
    #[inline]
    pub fn prev(&self) -> Option<NonNull<LinkedListNode>> {
        if ptr::eq(self.prev.as_ptr(), self) {
            None
        } else {
            Some(unsafe { NonNull::new_unchecked(self.prev.as_ptr() as *mut Self) })
        }
    }
}

// methods for list header
impl ListHead {
    /// Adds a node to the back of the list
    ///
    /// # Safety
    /// - The node must be unlinked (is_empty() == true)
    #[inline]
    pub fn push_back(self: Pin<&mut Self>, node: Pin<&mut LinkedListNode>) {
        debug_assert!(node.is_empty(), "node is not empty!");

        node.insert_before(self);
    }

    /// Adds a node to the front of the list
    ///
    /// # Safety
    /// - The node must be unlinked (is_empty() == true)
    #[inline]
    pub fn push_front(self: Pin<&mut Self>, node: Pin<&mut LinkedListNode>) {
        debug_assert!(node.is_empty(), "node is not empty!");

        node.insert_after(self);
    }

    /// Removes and returns the first node in the list
    #[inline]
    pub fn pop_front(self: Pin<&mut Self>) -> Option<NonNull<LinkedListNode>> {
        let next = self.next()?;
        // SAFETY: next is a valid pointer from our list
        unsafe {
            let next_ref = &mut *next.as_ptr();
            Pin::new_unchecked(next_ref).remove_from_list();
        }
        Some(next)
    }

    /// Removes and returns the last node in the list
    #[inline]
    pub fn pop_back(self: Pin<&mut Self>) -> Option<NonNull<LinkedListNode>> {
        let prev = self.prev()?;
        // SAFETY: prev is a valid pointer from our list
        unsafe {
            let prev_ref = &mut *prev.as_ptr();
            Pin::new_unchecked(prev_ref).remove_from_list();
        }
        Some(prev)
    }

    /// Checks if the list is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        ptr::eq(self.next.as_ptr(), self)
    }

    /// Counts the number of nodes in the list
    #[allow(dead_code)]
    pub fn size(&self) -> usize {
        let mut size = 0;
        let mut cur = self.next.clone();
        while !ptr::eq(self, cur.cur()) {
            cur = cur.next().clone();
            size += 1;
        }
        size
    }

    /// Gets the raw pointer to the node
    #[inline]
    pub fn as_ptr(&self) -> *const ListHead {
        self as *const ListHead
    }
}

#[pinned_drop]
impl PinnedDrop for ListHead {
    //#[inline]
    fn drop(self: Pin<&mut Self>) {
        if !self.is_empty() {
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

/// Connection relationship between list nodes
///
/// Uses `Cell` to provide interior mutability, allowing connection modifications
/// even with shared references
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

/// A safe doubly linked list implementation
///
/// This is a safe wrapper around `ListHead`, providing a more friendly interface
/// and complete ownership semantics
pub struct LinkedList<T> {
    inner: Pin<Box<ListHead>>,
    len: usize,
    _marker: PhantomData<T>,
}

/// Node type for storing actual data
///
/// Contains a `ListHead` for list connectivity and the actual stored data
#[pin_data(PinnedDrop)]
pub struct Node<T> {
    #[pin]
    head: LinkedListNode,
    data: T,
}

#[pinned_drop]
impl<T> PinnedDrop for Node<T> {
    fn drop(self: Pin<&mut Self>) {
        // SAFETY: we have a pinned reference to self
        let this = unsafe { self.get_unchecked_mut() };
        // SAFETY: head is pinned as part of self
        unsafe { Pin::new_unchecked(&mut this.head).remove_from_list() };
    }
}

impl<T> Node<T> {
    #[inline]
    pub fn new(data: T) -> impl PinInit<Self> {
        pin_init!(Self {
            head <- ListHead::new(),
            data,
        })
    }

    /// Take ownership of the data stored in the node.
    ///
    /// # Safety
    ///
    /// After calling this method, the data field will be in an uninitialized state.
    /// Any subsequent use of the node's data (through Deref/DerefMut) will be undefined behavior.
    /// The node should be dropped immediately after calling this method.
    #[inline]
    unsafe fn take_data(mut self: Pin<&mut Self>) -> T {
        // SAFETY: data is not pin, so we can move it out
        // After this call, self.data is in an uninitialized state
        ptr::read(&self.as_mut().get_unchecked_mut().data)
    }
}

impl<T> Deref for Node<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T> DerefMut for Node<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl<T> Default for LinkedList<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> LinkedList<T> {
    /// Creates a new empty list
    pub fn new() -> Self {
        Self {
            inner: Box::pin_init(ListHead::new()).unwrap(),
            len: 0,
            _marker: PhantomData,
        }
    }

    /// Returns the number of elements in the list
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Checks if the list is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Adds an element to the back of the list
    pub fn push_back(&mut self, data: T) {
        let node = Box::pin_init(Node::new(data)).unwrap();
        let node_box = unsafe { Pin::into_inner_unchecked(node) };
        let node_ptr = Box::into_raw(node_box);
        let node_ref = unsafe { &mut *node_ptr };

        // SAFETY: node_ref is valid and pinned
        unsafe {
            Pin::as_mut(&mut self.inner).push_back(Pin::new_unchecked(&mut node_ref.head));
        }
        self.len += 1;
    }

    /// Adds an element to the front of the list
    pub fn push_front(&mut self, data: T) {
        let node = Box::pin_init(Node::new(data)).unwrap();
        let node_box = unsafe { Pin::into_inner_unchecked(node) };
        let node_ptr = Box::into_raw(node_box);
        let node_ref = unsafe { &mut *node_ptr };

        // SAFETY: node_ref is valid and pinned
        unsafe {
            Pin::as_mut(&mut self.inner).push_front(Pin::new_unchecked(&mut node_ref.head));
        }
        self.len += 1;
    }

    /// Removes and returns the last element in the list
    pub fn pop_back(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }

        let tail = self.inner.prev()?;
        // SAFETY:
        // 1. head pointer comes from our Node
        // 2. node will be dropped immediately after take_data
        unsafe {
            let mut node = Box::from_raw((tail.as_ptr() as *mut ListHead).cast::<Node<T>>());
            Pin::new_unchecked(&mut node.head).remove_from_list();
            self.len -= 1;
            Some(Pin::new_unchecked(&mut *node).take_data())
        }
    }

    /// Removes and returns the first element in the list
    pub fn pop_front(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }

        let head = self.inner.next()?;
        // SAFETY:
        // 1. head pointer comes from our Node
        // 2. node will be dropped immediately after take_data
        unsafe {
            let mut node = Box::from_raw((head.as_ptr() as *mut ListHead).cast::<Node<T>>());
            Pin::new_unchecked(&mut node.head).remove_from_list();
            self.len -= 1;
            Some(Pin::new_unchecked(&mut *node).take_data())
        }
    }

    /// Clears the list, removing all elements
    pub fn clear(&mut self) {
        while self.pop_front().is_some() {}
    }

    /// Returns an iterator over the list's elements
    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            head: self.inner.next(),
            len: self.len,
            _marker: PhantomData,
        }
    }

    /// Returns a mutable iterator over the list's elements
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        IterMut {
            head: self.inner.next(),
            len: self.len,
            _marker: PhantomData,
        }
    }
}

impl<T> Drop for LinkedList<T> {
    fn drop(&mut self) {
        self.clear();
    }
}

/// An immutable iterator over the list
///
/// Allows accessing list elements through shared references
pub struct Iter<'a, T: 'a> {
    head: Option<NonNull<ListHead>>,
    len: usize,
    _marker: PhantomData<&'a T>,
}

impl<'a, T: 'a> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.len == 0 {
            return None;
        }

        let current = self.head?;
        // SAFETY: The pointer is valid as it comes from the list
        unsafe {
            let node = (current.as_ptr() as *const ListHead)
                .cast::<Node<T>>()
                .as_ref()?;
            self.head = node.head.next();
            self.len -= 1;
            Some(&**node)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

/// A mutable iterator over the list
///
/// Allows modifying list elements
pub struct IterMut<'a, T: 'a> {
    head: Option<NonNull<ListHead>>,
    len: usize,
    _marker: PhantomData<&'a mut T>,
}

impl<'a, T: 'a> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.len == 0 {
            return None;
        }

        let current = self.head?;
        // SAFETY: The pointer is valid as it comes from the list
        unsafe {
            let node = (current.as_ptr() as *mut ListHead)
                .cast::<Node<T>>()
                .as_mut()?;
            self.head = node.head.next();
            self.len -= 1;
            Some(&mut **node)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

unsafe impl Send for LinkedListNode {}
unsafe impl Sync for LinkedListNode {}

#[cfg(test)]
mod tests {
    extern crate alloc;
    extern crate test;

    use alloc::boxed::Box;
    use test::Bencher;

    use super::*;

    /// Creates a new empty list head
    fn new_double_list() -> Pin<Box<ListHead>> {
        Box::pin_init(ListHead::new()).expect("Failed to create ListHead")
    }

    /// Creates a new empty list node
    fn new_list_node() -> Pin<Box<LinkedListNode>> {
        Box::pin_init(ListHead::new()).expect("Failed to create ListHead")
    }

    #[test]
    fn test_empty_list() {
        let list = new_double_list();
        assert!(list.is_empty());
        assert_eq!(list.size(), 0);
        assert!(list.next().is_none());
        assert!(list.prev().is_none());
    }

    #[test]
    fn test_insert_next() {
        let mut list = new_double_list();
        let mut node1 = new_list_node();
        let mut node2 = new_list_node();

        Pin::as_mut(&mut list).push_back(node1.as_mut());
        assert!(!list.is_empty());
        assert_eq!(list.size(), 1);

        Pin::as_mut(&mut list).push_back(node2.as_mut());
        assert_eq!(list.size(), 2);

        let first = list.next().unwrap();
        let second = unsafe { first.as_ref().next().unwrap() };
        let third = unsafe { second.as_ref().next().unwrap() };
        assert!(ptr::eq(third.as_ptr(), list.as_ref().as_ptr()));
    }

    #[test]
    fn test_insert_prev() {
        let mut list = new_double_list();
        let mut node1 = new_list_node();
        let mut node2 = new_list_node();

        Pin::as_mut(&mut list).push_front(node1.as_mut());
        assert!(!list.is_empty());
        assert_eq!(list.size(), 1);

        Pin::as_mut(&mut list).push_front(node2.as_mut());
        assert_eq!(list.size(), 2);

        let last = list.prev().unwrap();
        let second_last = unsafe { last.as_ref().prev().unwrap() };
        let third_last = unsafe { second_last.as_ref().prev().unwrap() };
        assert!(ptr::eq(third_last.as_ptr(), list.as_ref().as_ptr()));
    }

    #[test]
    fn test_remove() {
        let mut list = new_double_list();
        let mut node1 = new_list_node();
        let mut node2 = new_list_node();

        Pin::as_mut(&mut list).push_back(node1.as_mut());
        Pin::as_mut(&mut list).push_back(node2.as_mut());
        assert_eq!(list.size(), 2);

        // remove node2
        Pin::as_mut(&mut node2).remove_from_list();
        assert_eq!(list.size(), 1);

        let next = list.next().unwrap();
        assert!(ptr::eq(next.as_ptr(), node1.as_ptr()));

        // remove node1
        Pin::as_mut(&mut node1).remove_from_list();
        assert!(list.is_empty());
        assert_eq!(list.size(), 0);
    }

    #[test]
    fn test_circular_operations() {
        let mut list = new_double_list();
        let mut node1 = new_list_node();
        let mut node2 = new_list_node();

        Pin::as_mut(&mut list).push_back(node1.as_mut());
        Pin::as_mut(&mut list).push_back(node2.as_mut());

        let first = list.next().unwrap();
        let second = unsafe { first.as_ref().next().unwrap() };
        let third = unsafe { second.as_ref().next().unwrap() };

        assert!(ptr::eq(third.as_ptr(), list.as_ref().as_ptr()));
    }

    #[test]
    fn test_multiple_operations() {
        let mut list = new_double_list();
        let mut node1 = new_list_node();
        let mut node2 = new_list_node();
        let mut node3 = new_list_node();

        Pin::as_mut(&mut list).push_back(node1.as_mut());
        Pin::as_mut(&mut list).push_front(node2.as_mut());
        Pin::as_mut(&mut list).push_back(node3.as_mut());
        assert_eq!(list.size(), 3);
        Pin::as_mut(&mut node3).remove_from_list();
        assert_eq!(list.size(), 2);
        Pin::as_mut(&mut node2).remove_from_list();
        assert_eq!(list.size(), 1);
        assert!(!list.is_empty());
        assert!(list.next().is_some());
        assert!(list.prev().is_some());
    }

    #[test]
    fn test_basic_operations() {
        let mut list = LinkedList::new();
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);

        // Test push operations
        list.push_back(1);
        list.push_back(2);
        list.push_front(0);
        assert_eq!(list.len(), 3);

        // Test iteration
        let mut iter = list.iter();
        assert_eq!(*iter.next().unwrap(), 0);
        assert_eq!(*iter.next().unwrap(), 1);
        assert_eq!(*iter.next().unwrap(), 2);
        assert_eq!(iter.next(), None);

        // Test pop operations
        assert_eq!(list.pop_back(), Some(2));
        assert_eq!(list.pop_front(), Some(0));
        assert_eq!(list.pop_back(), Some(1));
        assert!(list.is_empty());
    }

    #[test]
    fn test_mutable_iteration() {
        let mut list = LinkedList::new();
        list.push_back(1);
        list.push_back(2);
        list.push_back(3);

        // Modify elements through mutable iteration
        for x in list.iter_mut() {
            *x *= 2;
        }

        // Verify modifications
        let values: Vec<i32> = list.iter().copied().collect();
        assert_eq!(values, vec![2, 4, 6]);
    }

    #[test]
    fn test_memory_management() {
        let mut list = LinkedList::new();

        for i in 0..1000 {
            list.push_back(i);
            list.push_front(-i);
        }
        assert_eq!(list.len(), 2000);
        for _ in 0..1000 {
            list.pop_back();
        }
        assert_eq!(list.len(), 1000);
        drop(list);
    }

    #[test]
    fn test_clear() {
        let mut list = LinkedList::new();
        for i in 0..10 {
            list.push_back(i);
        }
        assert_eq!(list.len(), 10);
        list.clear();
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
        assert!(list.iter().next().is_none());
    }

    #[test]
    fn test_push_pop_alternating() {
        let mut list = LinkedList::new();
        for i in 0..100 {
            list.push_back(i);
            list.push_front(-i);
            assert_eq!(list.pop_back(), Some(i));
            assert_eq!(list.pop_front(), Some(-i));
            assert!(list.is_empty());
        }
    }

    #[test]
    fn test_iterator_count() {
        let mut list = LinkedList::new();
        let test_len = 100;

        for i in 0..test_len {
            list.push_back(i);
        }
        let iter = list.iter();
        assert_eq!(iter.size_hint(), (test_len, Some(test_len)));

        let count = iter.count();
        assert_eq!(count, test_len);
    }

    #[test]
    fn test_double_ended_iteration() {
        let mut list = LinkedList::new();

        for i in 0..5 {
            list.push_back(i);
        }

        let mut values: Vec<i32> = Vec::new();
        let mut front = true;
        while values.len() < 5 {
            if front {
                if let Some(value) = list.pop_front() {
                    values.push(value);
                }
            } else {
                if let Some(value) = list.pop_back() {
                    values.push(value);
                }
            }
            front = !front;
        }

        assert_eq!(values, vec![0, 4, 1, 3, 2]);
        assert!(list.is_empty());
    }

    #[test]
    fn test_stress() {
        let mut list = LinkedList::new();
        let operations = 10000;

        for i in 0..operations {
            match i % 4 {
                0 => list.push_back(i),
                1 => list.push_front(i),
                2 => {
                    let _ = list.pop_back();
                }
                3 => {
                    let _ = list.pop_front();
                }
                _ => unreachable!(),
            }
        }

        list.clear();
        assert!(list.is_empty());
    }

    #[bench]
    fn bench_ops(b: &mut Bencher) {
        b.iter(|| {
            let mut list = LinkedList::new();
            let operations = 1 << 16;

            for i in 0..operations {
                match i % 4 {
                    0 => list.push_back(i),
                    1 => list.push_front(i),
                    2 => {
                        let _ = list.pop_back();
                    }
                    3 => {
                        let _ = list.pop_front();
                    }
                    _ => unreachable!(),
                }
            }

            list.clear();
            assert!(list.is_empty());
        });
    }

    #[test]
    fn test_iterator_mut_modifications() {
        let mut list = LinkedList::new();

        for i in 0..5 {
            list.push_back(i);
        }

        for x in list.iter_mut() {
            *x *= 2;
        }

        let mut iter = list.iter();
        for i in 0..5 {
            assert_eq!(*iter.next().unwrap(), i * 2);
        }
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_remove_next_prev() {
        let mut list = new_double_list();
        let mut node1 = new_list_node();
        let mut node2 = new_list_node();
        let mut node3 = new_list_node();

        Pin::as_mut(&mut list).push_back(node1.as_mut());
        Pin::as_mut(&mut list).push_back(node2.as_mut());
        Pin::as_mut(&mut list).push_back(node3.as_mut());
        assert_eq!(list.size(), 3);

        // Test pop_front
        let removed = Pin::as_mut(&mut list).pop_front().unwrap();
        assert!(ptr::eq(removed.as_ptr(), node1.as_ref().get_ref()));
        assert_eq!(list.size(), 2);

        // Test pop_back
        let removed = Pin::as_mut(&mut list).pop_back().unwrap();
        assert!(ptr::eq(removed.as_ptr(), node3.as_ref().get_ref()));
        assert_eq!(list.size(), 1);

        // Verify remaining node
        let remaining = list.next().unwrap();
        assert!(ptr::eq(remaining.as_ptr(), node2.as_ref().get_ref()));
    }
}
