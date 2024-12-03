extern crate alloc;

use alloc::boxed::Box;
use core::mem;
use core::ops::Drop;

pub struct Node<T> {
    next: Link<T>,
    val: T,
}

pub struct List<T> {
    head: Link<T>,
    size: usize,
}

impl<T> Default for List<T> {
    fn default() -> Self {
        List::<T> {
            head: None,
            size: 0,
        }
    }
}

impl<T> Drop for List<T> {
    fn drop(&mut self) {
        while !self.is_empty() {
            self.pop();
        }
    }
}

impl<T> List<T> {
    pub const fn new() -> Self {
        List::<T> {
            head: None,
            size: 0,
        }
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn is_empty(&self) -> bool {
        assert_eq!(self.head.is_none(), self.size == 0);
        self.size == 0
    }

    pub fn push(&mut self, val: T) -> &mut Self {
        let mut new_node = Box::new(Node::<T>::new(val));
        let old_head = self.head.take();
        new_node.next = old_head;
        self.head = Some(new_node);
        self.size += 1;
        self
    }

    pub fn pop(&mut self) -> Option<T> {
        match self.head.take() {
            None => None,
            Some(mut old_head) => {
                assert!(self.size > 0);
                mem::swap(&mut self.head, &mut old_head.next);
                self.size -= 1;
                Some(old_head.take())
            }
        }
    }
}

type Link<T> = Option<Box<Node<T>>>;

// A safe singly linked list. External code should **NOT** rely on the address of a node in the list.
impl<T> Node<T> {
    const fn new(val: T) -> Self {
        Self {
            next: None,
            val,
        }
    }
    #[allow(dead_code)]
    fn as_ref(&self) -> &T {
        &self.val
    }
    #[allow(dead_code)]
    fn as_mut(&mut self) -> &mut T {
        &mut self.val
    }

    fn take(self) -> T {
        self.val
    }

    // O(1) insertion.
    #[allow(dead_code)]
    fn insert(&mut self, val: T) -> &mut Self {
        let mut new_node = Box::<Node<T>>::new(Node::<T> {
            next: None,
            val,
        });
        mem::swap(&mut new_node.val, &mut self.val);
        mem::swap(&mut new_node.next, &mut self.next);
        let old_next = mem::replace(&mut self.next, Some(new_node));
        assert!(old_next.is_none());
        self
    }

    // O(1) removal.
    #[allow(dead_code)]
    fn remove(&mut self) -> Option<Self> {
        match self.next.take() {
            None => None,
            Some(mut old_next) => {
                mem::swap(&mut old_next.next, &mut self.next);
                assert!(old_next.next.is_none());
                mem::swap(&mut old_next.val, &mut self.val);
                Some(*old_next)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate test;

    use super::*;
    use test::{black_box, Bencher};

    #[test]
    fn create_new_list() {
        let head = Node::<i32> {
            val: -1,
            next: None,
        };
        assert_eq!(head.val, -1);
    }

    #[test]
    fn insert_node() {
        let mut head = Node::<i32> {
            val: -1,
            next: None,
        };
        let new_node = head.insert(1024);
        assert_eq!(new_node.val, 1024);
        assert_eq!(new_node.next.as_ref().unwrap().val, -1);
    }

    #[test]
    fn remove_node() {
        let mut n0 = Node::<i32>::new(-1);
        let n1 = n0.insert(1024);
        assert_eq!(n1.val, 1024);
        let n2 = n1.remove();
        assert_eq!(n1.val, -1);
        assert!(n2.is_some());
        assert_eq!(n2.unwrap().val, 1024);
    }

    #[test]
    fn make_sequence_and_count() {
        let mut head = Node::<i32>::new(0);
        for i in 1..1024 {
            head.insert(i);
        }
        let mut count = 1;
        let mut current = &mut head;
        while current.next.is_some() {
            current = current.next.as_mut().unwrap();
            count += 1;
            assert_eq!(current.val, 1024 - count);
        }
        assert_eq!(count, 1024);
    }

    #[test]
    fn make_sequence_and_remove() {
        let mut head = Node::<i32>::new(0);
        for i in 1..1024 {
            head.insert(i);
        }
        let mut count = 0;
        while head.next.is_some() {
            head.remove();
            count += 1;
        }
        assert_eq!(count, 1023);
    }

    #[bench]
    fn bench_make_sequence_and_remove(b: &mut Bencher) {
        let n = 1 << 16;
        let mut head = Node::<i32>::new(0);
        b.iter(|| {
            for i in 1..n {
                black_box(head.insert(i));
            }
            while head.next.is_some() {
                black_box(head.remove());
            }
        });
    }

    #[test]
    fn test_push() {
        let n = 1usize << 16;
        let mut l = List::<usize>::default();
        for i in 0..n {
            l.push(i);
        }
        assert_eq!(l.size(), n);
    }

    // This test indicates the List should implement Drop trait itself to destory.
    // Or the destruction procedure will hit stackoverflow, since the boxes are
    // destroyed recursively.
    #[bench]
    fn bench_push(b: &mut Bencher) {
        let n = 1usize << 16;
        let mut l = List::<usize>::default();
        let mut count = 0;
        b.iter(|| {
            count += 1;
            for i in 0..n {
                black_box(l.push(i));
            }
        });
        assert_eq!(count * n, l.size());
    }

    #[bench]
    fn bench_push_and_pop(b: &mut Bencher) {
        let n = 1usize << 16;
        let mut l = List::<usize>::default();
        b.iter(|| {
            for i in 0..n {
                black_box(l.push(i));
            }
            assert_eq!(l.size(), n);
            while !l.is_empty() {
                black_box(l.pop());
            }
            assert!(l.is_empty());
        });
    }
}
