/// SpinArc is an alternative to Arc<Rwlock> aiming to achieve low
/// memory footprint while keeping memory safety and concurrency
/// safety. Currently it offers only strong_count for reference counting.
extern crate alloc;

use crate::{
    tinyarc::TinyArc as Arc,
    tinyrwlock::{RwLock, RwLockWriteGuard as WriteGuard},
};
use alloc::boxed::Box;
use core::{
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

pub type SpinArc<T> = Arc<RwLock<T>>;
type Uint = u8;

// Can be used to implement intrusive list based on fine grained rwlock.
#[derive(Default, Debug)]
pub struct IlistNode<T: Sized> {
    prev: Option<SpinArc<IlistNode<T>>>,
    next: Option<SpinArc<IlistNode<T>>>,
    // Make it Option<NonNull<T>> so that we can implement sentinel
    // node easier.
    object: Option<NonNull<T>>,
    // To avoid ABA problem.
    version: Uint,
}

impl<T> IlistNode<T> {
    pub fn version(&self) -> usize {
        self.version as usize
    }

    // Version is incremented if this node is inserted to or detached from a list.
    pub fn increment_version(&mut self) -> &mut Self {
        self.version += 1;
        self
    }

    pub const unsafe fn const_new(object: &'static T) -> Self {
        Self {
            version: 0,
            prev: None,
            next: None,
            object: Some(NonNull::from_ref(object)),
        }
    }

    pub const fn default() -> Self {
        Self {
            version: 0,
            prev: None,
            next: None,
            object: None,
        }
    }

    pub fn new(object: T) -> Self {
        let x: Box<_> = Box::new(object);
        Self {
            version: 0,
            prev: None,
            next: None,
            object: Some(Box::leak(x).into()),
        }
    }

    pub fn is_detached(&self) -> bool {
        self.prev.is_none() && self.next.is_none()
    }

    pub fn next(&self) -> Option<&SpinArc<Self>> {
        self.next.as_ref()
    }

    pub fn prev(&self) -> Option<&SpinArc<Self>> {
        self.prev.as_ref()
    }

    // Assume guards are acquired.
    fn do_detach(&mut self, prev: Option<&mut Self>, next: Option<&mut Self>) {
        if let Some(prev) = prev {
            let _ = core::mem::replace(&mut prev.next, None);
            core::mem::swap(&mut prev.next, &mut self.next);
        };
        if let Some(next) = next {
            let _ = core::mem::replace(&mut next.prev, None);
            core::mem::swap(&mut next.prev, &mut self.prev);
        }
        self.next = None;
        self.prev = None;
        debug_assert!(self.is_detached());
    }

    pub fn versioned_detach(my_version: Option<usize>, me: &mut SpinArc<Self>) -> bool {
        // FIXME: We are using a stupid algorithm now. When we are unable to
        // get all locks we need, we rollback.
        loop {
            let Some(mut write_me_guard) = me.try_write() else {
                core::hint::spin_loop();
                continue;
            };
            if write_me_guard.is_detached() {
                return false;
            }
            if let Some(version) = my_version {
                if version != write_me_guard.version() {
                    return false;
                }
            };
            let prev = write_me_guard.prev().map(|prev| prev.clone());
            let mut write_prev_guard = None;
            if prev.is_some() {
                write_prev_guard = unsafe { prev.as_ref().unwrap_unchecked() }.try_write();
                if write_prev_guard.is_none() {
                    core::hint::spin_loop();
                    continue;
                }
            }
            let next = write_me_guard.next().map(|next| next.clone());
            let mut write_next_guard = None;
            if next.is_some() {
                write_next_guard = unsafe { next.as_ref().unwrap_unchecked() }.try_write();
                if write_next_guard.is_none() {
                    core::hint::spin_loop();
                    continue;
                }
            }
            write_me_guard.do_detach(
                write_prev_guard.as_mut().map(|w| w.deref_mut()),
                write_next_guard.as_mut().map(|w| w.deref_mut()),
            );
            write_me_guard.increment_version();
            return true;
        }
    }

    pub fn detach(me: &mut SpinArc<Self>) -> bool {
        Self::versioned_detach(None, me)
    }

    pub fn versioned_insert_before(
        other_version: Option<usize>,
        other: &mut SpinArc<Self>,
        me: SpinArc<Self>,
    ) -> bool {
        loop {
            let Some(mut write_me_guard) = me.try_write() else {
                core::hint::spin_loop();
                continue;
            };
            if !write_me_guard.is_detached() {
                return false;
            }
            let Some(mut write_other_guard) = other.try_write() else {
                core::hint::spin_loop();
                continue;
            };
            if let Some(version) = other_version {
                if write_other_guard.version() != version {
                    return false;
                }
            };
            let prev = write_other_guard.prev.as_ref().map(|prev| prev.clone());
            let write_prev_guard = {
                if let Some(prev) = prev.as_ref() {
                    if let Some(guard) = prev.try_write() {
                        Some(guard)
                    } else {
                        core::hint::spin_loop();
                        continue;
                    }
                } else {
                    None
                }
            };
            // Now we have acquired all guards.
            let prev = core::mem::replace(&mut write_other_guard.prev, Some(me.clone()));
            let _ = core::mem::replace(&mut write_me_guard.prev, prev);
            if let Some(mut guard) = write_prev_guard {
                let _ = core::mem::replace(&mut guard.next, Some(me.clone()));
            };
            drop(write_other_guard);
            let _ = core::mem::replace(&mut write_me_guard.next, Some(other.clone()));
            write_me_guard.increment_version();
            return true;
        }
    }

    pub fn insert_before(other: &mut SpinArc<Self>, me: SpinArc<Self>) -> bool {
        Self::versioned_insert_before(None, other, me)
    }

    pub fn versioned_insert_after(
        other_version: Option<usize>,
        other: &mut SpinArc<Self>,
        me: SpinArc<Self>,
    ) -> bool {
        loop {
            let Some(mut write_me_guard) = me.try_write() else {
                core::hint::spin_loop();
                continue;
            };
            if !write_me_guard.is_detached() {
                return false;
            }
            let Some(mut write_other_guard) = other.try_write() else {
                core::hint::spin_loop();
                continue;
            };
            if let Some(version) = other_version {
                if write_other_guard.version() != version {
                    return false;
                }
            };
            let next = write_other_guard.next.as_ref().map(|next| next.clone());
            let write_next_guard = {
                if let Some(next) = next.as_ref() {
                    if let Some(guard) = next.try_write() {
                        Some(guard)
                    } else {
                        core::hint::spin_loop();
                        continue;
                    }
                } else {
                    None
                }
            };
            // Now we have acquired all guards.
            let next = core::mem::replace(&mut write_other_guard.next, Some(me.clone()));
            let _ = core::mem::replace(&mut write_me_guard.next, next);
            if let Some(mut guard) = write_next_guard {
                let _ = core::mem::replace(&mut guard.prev, Some(me.clone()));
            };
            drop(write_other_guard);
            let _ = core::mem::replace(&mut write_me_guard.prev, Some(other.clone()));
            write_me_guard.increment_version();
            return true;
        }
    }

    pub fn insert_after(other: &mut SpinArc<Self>, me: SpinArc<Self>) -> bool {
        Self::versioned_insert_after(None, other, me)
    }

    pub fn remove_after(me: &mut SpinArc<Self>) -> Option<SpinArc<Self>> {
        loop {
            let Some(w) = me.try_write() else {
                core::hint::spin_loop();
                continue;
            };
            if w.next().is_none() {
                return None;
            }
            let Some(wn) = (unsafe { w.next().as_ref().unwrap_unchecked().try_write() }) else {
                core::hint::spin_loop();
                continue;
            };
            let version = wn.version();
            drop(wn);
            let mut next = unsafe { w.next().unwrap_unchecked().clone() };
            drop(w);
            if Node::versioned_detach(Some(version), &mut next) {
                return Some(next);
            }
        }
    }
}

impl<T> Deref for IlistNode<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.object.as_ref().unwrap().as_ref() }
    }
}

impl<T> DerefMut for IlistNode<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.object.as_mut().unwrap().as_mut() }
    }
}

impl<T> Drop for IlistNode<T> {
    fn drop(&mut self) {
        // Static data should never reach here.
        self.object.map(|v| {
            let x = unsafe { Box::from_raw(v.as_ptr()) };
            drop(x);
        });
    }
}

#[allow(unused)]
pub struct MutexIter<'a, T> {
    mutex: WriteGuard<'a, IlistNode<T>>,
    current: Option<SpinArc<IlistNode<T>>>,
}

impl<'a, T> MutexIter<'a, T> {
    pub fn new(head: &'a SpinArc<IlistNode<T>>) -> Self {
        let mutex = head.write();
        let current = mutex.next().map(|v| v.clone());
        Self { mutex, current }
    }
}

impl<'a, T> Iterator for MutexIter<'a, T> {
    type Item = SpinArc<IlistNode<T>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current.is_none() {
            return None;
        }
        let x = unsafe {
            self.current
                .as_ref()
                .unwrap_unchecked()
                .read()
                .next()
                .map(|v| v.clone())
        };
        return core::mem::replace(&mut self.current, x);
    }
}

pub struct VerIter<T> {
    current: SpinArc<IlistNode<T>>,
    version: Uint,
}

impl<T> VerIter<T> {
    pub fn new(head: &SpinArc<IlistNode<T>>) -> Self {
        let r = head.read();
        let version = r.version() as Uint;
        let current = head.clone();
        Self { version, current }
    }
}

impl<T> Iterator for VerIter<T> {
    type Item = (usize, SpinArc<IlistNode<T>>);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let Some(r) = self.current.try_read() else {
                core::hint::spin_loop();
                continue;
            };
            if r.version() != self.version.into() {
                return None;
            }
            let Some(next) = r.next().map(|v| v.clone()) else {
                return None;
            };
            let Some(rn) = next.try_read() else {
                core::hint::spin_loop();
                continue;
            };
            let version = rn.version();
            drop(rn);
            drop(r);
            self.current = next;
            self.version = version as Uint;
            return Some((self.version as usize, self.current.clone()));
        }
    }
}

pub struct Ilist<T: Sized> {
    // FIXME: We can use only one sentinel node if our IlistNode impl
    // is aliasing awared.
    head: SpinArc<IlistNode<T>>,
    tail: SpinArc<IlistNode<T>>,
}

type Node<T> = IlistNode<T>;

impl<T> Ilist<T> {
    pub fn default() -> Self {
        let mut head = Arc::new(RwLock::new(Node::<T>::default()));
        let tail = Arc::new(RwLock::new(Node::<T>::default()));
        Node::<T>::insert_after(&mut head, tail.clone());
        Self {
            head: head,
            tail: tail,
        }
    }

    #[inline]
    fn head(&self) -> &SpinArc<Node<T>> {
        &self.head
    }

    #[inline]
    fn head_mut(&mut self) -> &mut SpinArc<Node<T>> {
        &mut self.head
    }

    #[inline]
    fn tail(&self) -> &SpinArc<Node<T>> {
        &self.tail
    }

    #[inline]
    fn tail_mut(&mut self) -> &mut SpinArc<Node<T>> {
        &mut self.tail
    }

    pub fn is_empty(&self) -> bool {
        self.head()
            .read()
            .next()
            .map_or(false, |v| v.is(self.tail()))
    }

    pub fn push_back(&mut self, n: SpinArc<Node<T>>) {
        Node::<T>::insert_before(self.tail_mut(), n);
    }

    pub fn pop_front(&mut self) -> Option<SpinArc<Node<T>>> {
        Node::<T>::remove_after(self.head_mut())
    }
}

#[cfg(test)]
mod tests {
    extern crate test;
    use super::*;
    use std::{collections::HashSet, thread};
    use test::{black_box, Bencher};

    #[test]
    fn threaded_insert_after_many() {
        type Node = IlistNode<usize>;
        let head = Arc::new(RwLock::new(Node::new(0)));
        let n = 1024;
        let mut vt = Vec::new();
        for i in 1..n {
            let handle = |id, mut head| {
                let node = Arc::new(RwLock::new(Node::new(id)));
                Node::insert_after(&mut head, node);
            };
            let head = head.clone();
            let t = thread::spawn(move || handle(i, head));
            vt.push(t);
        }
        for t in vt {
            t.join().unwrap();
        }
        let mut cursor = Some(head);
        let mut counter = 0;
        let mut ids = HashSet::new();
        while cursor.is_some() {
            counter += 1;
            let id = **cursor.as_ref().unwrap().read();
            assert!(ids.get(&id).is_none());
            assert!(ids.insert(id));
            let tmp = cursor.unwrap().read().next.clone();
            cursor = tmp;
        }
        assert_eq!(counter, n);
        for i in 0..n {
            ids.get(&i).is_some();
        }
    }

    #[test]
    fn insert_after_many() {
        type Node = IlistNode<usize>;
        let head = Arc::new(RwLock::new(Node::new(0)));
        let mut prev = head.clone();
        for i in 1..1024 {
            let next = Arc::new(RwLock::new(Node::new(i)));
            Node::insert_after(&mut prev, next);
            let tmp = prev.read().next.as_ref().unwrap().clone();
            prev = tmp;
        }
        let mut cursor = Some(head);
        let mut counter = 0;
        while cursor.is_some() {
            assert_eq!(counter, **cursor.as_ref().unwrap().read());
            counter += 1;
            let tmp = cursor.unwrap().read().next.clone();
            cursor = tmp;
        }
    }

    #[test]
    fn insert_before_many() {
        type Node = IlistNode<usize>;
        let tail = Arc::new(RwLock::new(Node::new(0)));
        let mut me = tail.clone();
        for i in 1..1024 {
            let prev = Arc::new(RwLock::new(Node::new(i)));
            Node::insert_before(&mut me, prev);
            let tmp = me.read().prev.as_ref().unwrap().clone();
            me = tmp;
        }
        let mut cursor = Some(tail);
        let mut counter = 0;
        while cursor.is_some() {
            assert_eq!(counter, **cursor.as_ref().unwrap().read());
            counter += 1;
            let tmp = cursor.unwrap().read().prev.clone();
            cursor = tmp;
        }
    }

    #[test]
    fn detach_me() {
        type Node = IlistNode<usize>;
        let mut a = Arc::new(RwLock::new(Node::new(0)));
        let mut b = Arc::new(RwLock::new(Node::new(1)));
        let c = Arc::new(RwLock::new(Node::new(2)));
        Node::insert_after(&mut b, c.clone());
        assert!(b.read().prev.is_none());
        assert!(b.read().next.is_some());
        assert!(c.read().prev.is_some());
        assert!(c.read().next.is_none());
        // &b is not detached, so this action should fail.
        assert!(!Node::insert_after(&mut a, b.clone()));
        // &a is detached, so this action should succeed.
        Node::insert_before(&mut b, a.clone());
        assert!(a.read().next.is_some());
        assert!(a.read().prev.is_none());
        assert_eq!(**a.read().next.as_ref().unwrap().read(), 1);
        assert_eq!(**c.read().prev.as_ref().unwrap().read(), 1);
        Node::detach(&mut b);
        assert!(b.read().is_detached());
        assert_eq!(**a.read().next.as_ref().unwrap().read(), 2);
        assert_eq!(**c.read().prev.as_ref().unwrap().read(), 0);
    }

    #[bench]
    fn bench_insert_after_many(b: &mut Bencher) {
        b.iter(|| {
            let n = 1usize << 16;
            type Node = IlistNode<usize>;
            let head = Arc::new(RwLock::new(Node::new(0)));
            let mut prev = head.clone();
            for i in 1..n {
                let next = Arc::new(RwLock::new(Node::new(i)));
                black_box(Node::insert_after(&mut prev, next));
                let tmp = prev.read().next.as_ref().unwrap().clone();
                prev = tmp;
            }
        });
    }
}
