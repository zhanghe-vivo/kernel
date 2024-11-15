extern crate alloc;

use alloc::boxed::Box;
use core::mem;

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

    pub fn empty(&self) -> bool {
        self.size == 0
    }

    pub fn get_first(&mut self) -> Option<&mut Node<T>> {
        match &mut self.head {
            None => None,
            Some(fst) => Some(fst.as_mut()),
        }
    }

    pub fn push(&mut self, val: T) -> &mut Node<T> {
        self.size += 1;
        if self.head.is_none() {
            let fst = Box::<Node<T>>::new(Node::<T> {
                next: None,
                val: val,
            });
            self.head = Some(fst);
            return self.get_first().unwrap();
        }
        self.get_first().unwrap().insert(val)
    }

    pub fn pop(&mut self) -> Option<Node<T>> {
        match &mut self.head {
            None => None,
            Some(fst) => {
                self.size -= 1;
                fst.remove()
            }
        }
    }
}

type Link<T> = Option<Box<Node<T>>>;

// A safe singly linked list. External code should **NOT** rely on the address of a node in the list.
impl<T> Node<T> {
    // O(1) insertion.
    fn insert(&mut self, val: T) -> &mut Self {
        let mut new_node = Box::<Node<T>>::new(Node::<T> {
            next: None,
            val: val,
        });
        mem::swap(&mut new_node.val, &mut self.val);
        mem::swap(&mut new_node.next, &mut self.next);
        let old_next = mem::replace(&mut self.next, Some(new_node));
        assert!(old_next.is_none());
        self
    }

    // O(1) removal.
    fn remove(&mut self) -> Option<Self> {
        if self.next.is_none() {
            return None;
        }
        let mut old_next = mem::replace(&mut self.next, None).unwrap();
        assert!(self.next.is_none());
        mem::swap(&mut old_next.next, &mut self.next);
        assert!(old_next.next.is_none());
        mem::swap(&mut old_next.val, &mut self.val);
        Some(*old_next)
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
        let mut n0 = Node::<i32> {
            val: -1,
            next: None,
        };
        let n1 = n0.insert(1024);
        assert_eq!(n1.val, 1024);
        let n2 = n1.remove();
        assert_eq!(n1.val, -1);
        assert!(n2.is_some());
        assert_eq!(n2.unwrap().val, 1024);
    }

    #[test]
    fn make_sequence_and_count() {
        let mut head = Node::<i32> { val: 0, next: None };
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
        let mut head = Node::<i32> { val: 0, next: None };
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
        let mut head = Node::<i32> { val: 0, next: None };
        b.iter(|| {
            for i in 1..n {
                black_box(head.insert(i));
            }
            while head.next.is_some() {
                black_box(head.remove());
            }
        });
    }

    #[bench]
    fn bench_list_push_and_pop(b: &mut Bencher) {
        let n = 1 << 16;
        let mut l = List::<i32>::default();
        b.iter(|| {
            for i in 1..n {
                black_box(l.push(i));
            }
            while !l.empty() {
                black_box(l.pop());
            }
        });
    }
}
