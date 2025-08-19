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

extern crate alloc;
use crate::{
    intrusive::Adapter,
    list::typed_atomic_ilist::{
        AtomicListHead, AtomicListIterator as ListIterator,
        AtomicListReverseIterator as ListReverseIterator,
    },
};
use alloc::boxed::Box;
use core::{
    marker::PhantomData,
    ops::Deref,
    ptr::NonNull,
    sync::atomic::{fence, AtomicPtr, Ordering},
};

#[cfg(target_pointer_width = "32")]
type Uint = u8;
#[cfg(target_pointer_width = "32")]
type AtomicUint = core::sync::atomic::AtomicU8;

#[cfg(target_pointer_width = "64")]
type Uint = usize;
#[cfg(target_pointer_width = "64")]
type AtomicUint = core::sync::atomic::AtomicUsize;

#[derive(Debug)]
#[repr(C)]
pub struct TinyArcInner<T: Sized> {
    data: T,
    // We don't need a large counter as Arc, we don't have weak
    // counter either.
    rc: AtomicUint,
}

impl<T: Sized> TinyArcInner<T> {
    pub const fn const_new(data: T) -> Self {
        Self {
            data,
            rc: AtomicUint::new(1),
        }
    }

    pub const fn new(data: T) -> Self {
        Self::const_new(data)
    }
}

// We don't Send or Sync TinyArcInner directly. All TinyArcInner values should
// be static or allocated from the heap and wrapped in TinyArc.
unsafe impl<T> Send for TinyArcInner<T> {}
unsafe impl<T> Sync for TinyArcInner<T> {}

// Make it transparent so that we don't have extra space overhead when
// using Option<TinyArc<T>>.
// See https://rust-lang.github.io/unsafe-code-guidelines/layout/enums.html#discriminant-elision-on-option-like-enums.
// https://doc.rust-lang.org/nomicon/other-reprs.html#reprtransparent
#[derive(Debug)]
#[repr(transparent)]
pub struct TinyArc<T: Sized> {
    inner: NonNull<TinyArcInner<T>>,
}

impl<T: Default + Sized> Default for TinyArc<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> TinyArc<T> {
    #[inline]
    pub fn new(data: T) -> Self {
        let x = Box::new(TinyArcInner::const_new(data));
        assert_eq!(Box::as_ptr(&x) as usize % core::mem::align_of::<T>(), 0);
        Self {
            inner: unsafe { NonNull::new_unchecked(Box::into_raw(x)) },
        }
    }

    #[inline]
    pub const unsafe fn const_new(inner: &'static TinyArcInner<T>) -> Self {
        TinyArc {
            inner: NonNull::from_ref(inner),
        }
    }

    #[inline]
    pub unsafe fn from_inner(inner: NonNull<TinyArcInner<T>>) -> Self {
        inner.as_ref().rc.fetch_add(1, Ordering::Release);
        TinyArc { inner }
    }

    #[inline]
    pub unsafe fn get_handle(this: &Self) -> *const u8 {
        Self::as_ptr(this) as *const u8
    }

    #[inline]
    pub fn strong_count(this: &Self) -> usize {
        #[cfg(target_pointer_width = "32")]
        unsafe {
            this.inner.as_ref().rc.load(Ordering::Relaxed) as usize
        }
        #[cfg(target_pointer_width = "64")]
        unsafe {
            this.inner.as_ref().rc.load(Ordering::Relaxed)
        }
    }

    #[inline]
    pub unsafe fn increment_strong_count(this: &Self) {
        let old = this.inner.as_ref().rc.fetch_add(1, Ordering::Relaxed);
        assert_ne!(old, 0);
    }

    #[inline]
    pub unsafe fn decrement_strong_count(this: &Self) {
        let old = this.inner.as_ref().rc.fetch_sub(1, Ordering::Relaxed);
        assert_ne!(old, 1);
    }

    #[inline]
    pub fn is(&self, other: &Self) -> bool {
        unsafe { Self::get_handle(self) == Self::get_handle(other) }
    }

    #[must_use]
    pub fn as_ptr(this: &Self) -> *const T {
        let ptr: *mut TinyArcInner<T> = NonNull::as_ptr(this.inner);

        // SAFETY: This cannot go through Deref::deref because this is required to retain raw/mut provenance
        unsafe { &raw mut (*ptr).data }
    }

    pub fn into_raw(this: Self) -> *const T {
        let ptr = Self::as_ptr(&this);
        core::mem::forget(this);
        ptr
    }

    pub unsafe fn from_raw(ptr: *const T) -> Self {
        // SAFETY: ptr offset is same as TinyArcInner struct offset no recalculation of
        // offset is required
        TinyArc {
            inner: NonNull::new_unchecked(ptr as *mut TinyArcInner<T>),
        }
    }

    // `get_mut` requires `&mut Arc` which is different from what Sync
    // indicates. Thus it's impossible to see two threads `get_mut` successfully
    // at the same time.
    pub fn get_mut(this: &mut Self) -> Option<&mut T> {
        let rc = unsafe { this.inner.as_ref().rc.load(Ordering::Acquire) };
        if rc != 1 {
            return None;
        }
        Some(unsafe { &mut this.inner.as_mut().data })
    }

    #[inline]
    pub unsafe fn get_mut_unchecked(this: &mut Self) -> &mut T {
        &mut this.inner.as_mut().data
    }
}

impl<T: Sized> Clone for TinyArc<T> {
    #[inline]
    fn clone(&self) -> TinyArc<T> {
        let old = unsafe { self.inner.as_ref() }
            .rc
            .fetch_add(1, Ordering::Relaxed);
        assert!(old >= 1);
        TinyArc { inner: self.inner }
    }
}

impl<T: Sized> Drop for TinyArc<T> {
    #[inline]
    fn drop(&mut self) {
        let old_val = unsafe { self.inner.as_ref() }
            .rc
            .fetch_sub(1, Ordering::Acquire);
        if old_val != 1 {
            return;
        }
        fence(Ordering::SeqCst);
        // Static data should never reach here.
        let x = unsafe { Box::from_non_null(self.inner) };
        drop(x);
    }
}

impl<T: Sized> Deref for TinyArc<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe { &self.inner.as_ref().data }
    }
}

// TinyArc doesn't contain the value it manages, but a pointer to the value. We
// also assume no alias of the internal NonNull, so it's safe to impl Send +
// Sync for it.
unsafe impl<T: Sized> Send for TinyArc<T> {}
unsafe impl<T: Sized> Sync for TinyArc<T> {}

// This list is semi-safe for concurrency. When performing list operations, the
// lock on the whole list must be acquired first. Must be noted, when detaching
// a node from a list, we must be sure that the node being detached exactly
// belongs to the list we are locking.
#[derive(Default, Debug)]
pub struct TinyArcList<T: Sized, A: Adapter> {
    head: AtomicListHead<T, A>,
    tail: AtomicListHead<T, A>,
}

impl<T: Sized, A: Adapter> TinyArcList<T, A> {
    pub const fn const_new() -> Self {
        Self {
            head: AtomicListHead::<T, A>::new(),
            tail: AtomicListHead::<T, A>::new(),
        }
    }

    pub const fn new() -> Self {
        Self::const_new()
    }

    #[inline]
    pub fn init(&mut self) -> bool {
        AtomicListHead::<T, A>::insert_after(&mut self.head, &mut self.tail)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.head.next() == Some(NonNull::from_ref(&self.tail))
    }

    #[inline]
    pub fn list_head_of_mut(this: &mut TinyArc<T>) -> Option<&mut AtomicListHead<T, A>> {
        let this_val = TinyArc::<T>::get_mut(this)?;
        let ptr = this_val as *mut _ as *mut u8;
        let list_head_ptr = unsafe { ptr.add(A::offset()) as *mut AtomicListHead<T, A> };
        Some(unsafe { &mut *list_head_ptr })
    }

    #[inline]
    pub unsafe fn list_head_of_mut_unchecked(this: &mut TinyArc<T>) -> &mut AtomicListHead<T, A> {
        let this_val = TinyArc::<T>::get_mut_unchecked(this);
        let ptr = this_val as *mut _ as *mut u8;
        let list_head_ptr = ptr.add(A::offset()) as *mut AtomicListHead<T, A>;
        &mut *list_head_ptr
    }

    #[inline]
    pub unsafe fn make_arc_from(node: &AtomicListHead<T, A>) -> TinyArc<T> {
        let ptr = node as *const _ as *const u8;
        let mut offset = core::mem::offset_of!(TinyArcInner<T>, data);
        offset += A::offset();
        let inner = &*(ptr.sub(offset) as *const TinyArcInner<T>);
        TinyArc::from_inner(NonNull::from_ref(inner))
    }

    pub fn insert_after(other_node: &mut AtomicListHead<T, A>, mut me: TinyArc<T>) -> bool {
        let me_node = unsafe { Self::list_head_of_mut_unchecked(&mut me) };
        if !AtomicListHead::<T, A>::insert_after(other_node, me_node) {
            return false;
        }
        // The list shares ownership of me.
        core::mem::forget(me);
        true
    }

    pub fn insert_before(other_node: &mut AtomicListHead<T, A>, mut me: TinyArc<T>) -> bool {
        let me_node = unsafe { Self::list_head_of_mut_unchecked(&mut me) };
        if !AtomicListHead::<T, A>::insert_before(other_node, me_node) {
            return false;
        }
        // The list shares ownership of me.
        core::mem::forget(me);
        true
    }

    pub fn push_back(&mut self, me: TinyArc<T>) -> bool {
        if Self::insert_before(&mut self.tail, me) {
            return true;
        }
        false
    }

    pub fn back(&self) -> Option<TinyArc<T>> {
        if self.is_empty() {
            return None;
        }
        let Some(mut prev) = self.tail.prev() else {
            panic!("Tail's prev node should not be None");
        };
        Some(unsafe { Self::make_arc_from(prev.as_ref()) })
    }

    pub fn front(&self) -> Option<TinyArc<T>> {
        if self.is_empty() {
            return None;
        }
        let Some(mut next) = self.head.next() else {
            panic!("Head's next node should not be None");
        };
        Some(unsafe { Self::make_arc_from(next.as_ref()) })
    }

    pub fn pop_front(&mut self) -> Option<TinyArc<T>> {
        assert!(self.head.next().is_some());
        if self.is_empty() {
            return None;
        }
        let Some(mut next) = self.head.next() else {
            panic!("Head's next node should not be None");
        };
        let arc = unsafe { Self::make_arc_from(next.as_ref()) };
        let ok = AtomicListHead::<T, A>::detach(unsafe { next.as_mut() });
        assert!(ok);
        unsafe { TinyArc::<T>::decrement_strong_count(&arc) };
        Some(arc)
    }

    pub fn detach(me: &mut TinyArc<T>) -> bool {
        let me_node = unsafe { Self::list_head_of_mut_unchecked(me) };
        if !AtomicListHead::<T, A>::detach(me_node) {
            return false;
        }
        unsafe { TinyArc::<T>::decrement_strong_count(me) };
        true
    }

    pub fn clear(&mut self) -> usize {
        let mut c = 0;
        for mut i in
            TinyArcListIterator::<T, A>::new(&self.head, Some(NonNull::from_ref(&self.tail)))
        {
            Self::detach(&mut i);
            c += 1;
        }
        c
    }

    pub fn iter(&self) -> TinyArcListIterator<T, A> {
        TinyArcListIterator::<T, A>::new(&self.head, Some(NonNull::from_ref(&self.tail)))
    }
}

impl<T: Sized, A: Adapter> Drop for TinyArcList<T, A> {
    #[inline]
    fn drop(&mut self) {
        // NOTE: Elements should be cleared by calling `clear` method
        // since move occurs when dropping. Do you recall how drop is
        // called? It's `drop(val)`.
        // Maybe we can change `head` and `tail` to `Box<struct(AtomicListHead,AtomicListHead)>`,
        // which is implicitly pinned.
    }
}

pub struct TinyArcListIterator<T, A: Adapter> {
    it: ListIterator<T, A>,
}

pub struct TinyArcListReverseIterator<T, A: Adapter> {
    it: ListReverseIterator<T, A>,
}

impl<T, A: Adapter> TinyArcListIterator<T, A> {
    pub fn new(head: &AtomicListHead<T, A>, tail: Option<NonNull<AtomicListHead<T, A>>>) -> Self {
        Self {
            it: ListIterator::new(head, tail),
        }
    }
}

impl<T, A: Adapter> TinyArcListReverseIterator<T, A> {
    pub fn new(tail: &AtomicListHead<T, A>, head: Option<NonNull<AtomicListHead<T, A>>>) -> Self {
        Self {
            it: ListReverseIterator::new(tail, head),
        }
    }
}

impl<T, A: Adapter> Iterator for TinyArcListIterator<T, A> {
    type Item = TinyArc<T>;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.it.next()?;
        Some(unsafe { TinyArcList::<T, A>::make_arc_from(node.as_ref()) })
    }
}

impl<T, A: Adapter> Iterator for TinyArcListReverseIterator<T, A> {
    type Item = TinyArc<T>;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.it.next()?;
        Some(unsafe { TinyArcList::<T, A>::make_arc_from(node.as_ref()) })
    }
}

#[cfg(test)]
mod tests {
    extern crate test;
    use super::*;
    use crate::{
        impl_simple_intrusive_adapter, list::typed_atomic_ilist::AtomicListHead, tinyrwlock::RwLock,
    };
    use test::Bencher;

    impl_simple_intrusive_adapter!(OffsetOfCsl, Thread, control_status_list);
    impl_simple_intrusive_adapter!(OffsetOfTl, Thread, timer_list);

    #[derive(Default, Debug)]
    pub struct Thread {
        pub control_status_list: AtomicListHead<Thread, OffsetOfCsl>,
        pub timer_list: AtomicListHead<Thread, OffsetOfTl>,
        pub id: usize,
    }

    impl Thread {
        pub fn new(id: usize) -> Self {
            Self {
                id,
                ..Default::default()
            }
        }
    }

    #[test]
    fn test_basic() {
        type Ty = TinyArc<Thread>;
        type CslList = TinyArcList<Thread, OffsetOfCsl>;
        type TlList = TinyArcList<Thread, OffsetOfTl>;
        let mut t = TinyArc::new(Thread::default());
        assert_eq!(&t.control_status_list as *const _, unsafe {
            CslList::list_head_of_mut_unchecked(&mut t)
        } as *const _,);
        assert_eq!(
            &t.timer_list as *const _,
            unsafe { TlList::list_head_of_mut_unchecked(&mut t) } as *const _,
        );
    }

    #[test]
    fn test_detach_during_iter() {
        type Ty = TinyArc<Thread>;
        type CslList = TinyArcList<Thread, OffsetOfCsl>;
        type L = AtomicListHead<Thread, OffsetOfCsl>;
        let mut head = RwLock::new(L::default());
        let mut w = head.write();
        let t = TinyArc::new(Thread::default());
        CslList::insert_after(&mut *w, t);
        for mut e in TinyArcListIterator::new(&*w, None) {
            CslList::detach(&mut e);
        }
    }

    #[test]
    fn test_insert_and_detach() {
        type Ty = TinyArc<Thread>;
        type CslList = TinyArcList<Thread, OffsetOfCsl>;
        type L = AtomicListHead<Thread, OffsetOfCsl>;
        let n = 4;
        let mut head = L::default();
        for i in 0..n {
            let t = Ty::new(Thread::new(i));
            assert_eq!(Ty::strong_count(&t), 1);
            CslList::insert_after(&mut head, t.clone());
            assert_eq!(Ty::strong_count(&t), 2);
        }
        let mut counter = (n - 1) as isize;
        for mut i in TinyArcListIterator::new(&head, None) {
            assert_eq!(i.id, counter as usize);
            assert_eq!(Ty::strong_count(&i), 2);
            counter -= 1;
            assert!(CslList::detach(&mut i));
            assert_eq!(Ty::strong_count(&i), 1);
        }
    }

    #[test]
    fn test_insert_before_and_detach() {
        type Ty = TinyArc<Thread>;
        type CslList = TinyArcList<Thread, OffsetOfCsl>;
        type L = AtomicListHead<Thread, OffsetOfCsl>;
        let n = 4;
        let mut tail = L::default();
        for i in 0..n {
            let t = Ty::new(Thread::new(i));
            assert_eq!(Ty::strong_count(&t), 1);
            CslList::insert_before(&mut tail, t.clone());
            assert_eq!(Ty::strong_count(&t), 2);
        }
        let mut counter = (n - 1) as isize;
        for mut i in TinyArcListReverseIterator::new(&tail, None) {
            assert_eq!(i.id, counter as usize);
            assert_eq!(Ty::strong_count(&i), 2);
            counter -= 1;
            assert!(CslList::detach(&mut i));
            assert_eq!(Ty::strong_count(&i), 1);
        }
    }

    #[test]
    fn test_push_and_pop() {
        type Ty = TinyArc<Thread>;
        type CslList = TinyArcList<Thread, OffsetOfCsl>;
        type L = AtomicListHead<Thread, OffsetOfCsl>;
        let n = 16;
        let mut l = CslList::default();
        l.init();
        for i in 0..n {
            let mut t = Ty::new(Thread::new(i));
            assert_eq!(Ty::strong_count(&t), 1);
            let node = unsafe { CslList::list_head_of_mut_unchecked(&mut t) };
            assert!(node.is_detached());
            assert!(l.push_back(t.clone()));
            assert!(!CslList::insert_before(&mut l.tail, t.clone()));
            assert_eq!(Ty::strong_count(&t), 2);
        }
        for i in 0..n {
            let t = l.pop_front();
            assert!(t.is_some());
            let t = unsafe { t.unwrap_unchecked() };
            assert_eq!(t.id, i);
            assert_eq!(Ty::strong_count(&t), 1);
        }
        assert!(l.pop_front().is_none());
        assert!(l.is_empty());
    }

    #[test]
    fn test_push_and_drop() {
        type Ty = TinyArc<Thread>;
        type CslList = TinyArcList<Thread, OffsetOfCsl>;
        type L = AtomicListHead<Thread, OffsetOfCsl>;
        let n = 16;
        let mut l = CslList::default();
        l.init();
        for i in 0..n {
            let mut t = Ty::new(Thread::new(i));
            assert_eq!(Ty::strong_count(&t), 1);
            let node = unsafe { CslList::list_head_of_mut_unchecked(&mut t) };
            assert!(node.is_detached());
            assert!(l.push_back(t.clone()));
            assert!(!CslList::insert_before(&mut l.tail, t.clone()));
            assert_eq!(Ty::strong_count(&t), 2);
        }
        l.clear();
    }

    #[test]
    fn test_detach_during_iter_2() {
        type Ty = TinyArc<Thread>;
        type CslList = TinyArcList<Thread, OffsetOfCsl>;
        type L = AtomicListHead<Thread, OffsetOfCsl>;
        let mut l = CslList::default();
        l.init();
        let mut n = 16;
        for i in 0..n {
            let t = Ty::new(Thread::new(i));
            assert_eq!(Ty::strong_count(&t), 1);
            assert!(l.push_back(t.clone()));
        }

        loop {
            let mut iter = l.iter();
            if let Some(mut t) = iter.next() {
                assert_eq!(Ty::strong_count(&t), 2);
                assert!(CslList::detach(&mut t));
                assert_eq!(Ty::strong_count(&t), 1);
                n -= 1;
            } else {
                break;
            }
        }
        assert_eq!(n, 0);
    }

    #[test]
    fn test_detach_and_insert_during_iter() {
        type Ty = TinyArc<Thread>;
        type CslList = TinyArcList<Thread, OffsetOfCsl>;
        type L = AtomicListHead<Thread, OffsetOfCsl>;
        let mut l = CslList::default();
        l.init();
        let n = 16;
        for i in 0..n {
            let mut t = Ty::new(Thread::new(i));
            assert_eq!(Ty::strong_count(&t), 1);
            let node = unsafe { CslList::list_head_of_mut_unchecked(&mut t) };
            assert!(node.is_detached());
            assert!(l.push_back(t.clone()));
        }

        for i in 0..n {
            let mut iter = l.iter();
            if let Some(mut t) = iter.next() {
                assert_eq!(Ty::strong_count(&t), 2);
                assert!(CslList::detach(&mut t));
                assert_eq!(Ty::strong_count(&t), 1);
                // insert back to the list again
                l.push_back(t.clone());
            }
        }
        l.clear();
    }

    #[test]
    fn test_into_raw_and_from_raw() {
        type Ty = TinyArc<Thread>;
        let t = Ty::new(Thread::default());
        assert_eq!(Ty::strong_count(&t), 1);
        let ptr = Ty::into_raw(t);
        let t2 = unsafe { Ty::from_raw(ptr) };
        assert_eq!(Ty::strong_count(&t2), 1);
    }

    #[bench]
    fn bench_insert_and_detach(b: &mut Bencher) {
        type Ty = TinyArc<Thread>;
        type CslList = TinyArcList<Thread, OffsetOfCsl>;
        type L = AtomicListHead<Thread, OffsetOfCsl>;
        let n = 1 << 16;
        b.iter(|| {
            let mut head = L::default();
            let mut tail = L::default();
            L::insert_after(&mut head, &mut tail);
            for i in 0..n {
                let mut t = Ty::new(Thread::new(i));
                assert_eq!(Ty::strong_count(&t), 1);
                CslList::insert_after(&mut head, t.clone());
                assert_eq!(Ty::strong_count(&t), 2);
            }
            let mut counter = (n - 1) as isize;
            for mut i in TinyArcListIterator::new(&head, Some(NonNull::from_ref(&tail))) {
                assert_eq!(i.id, counter as usize);
                assert_eq!(Ty::strong_count(&i), 2);
                counter -= 1;
                assert!(CslList::detach(&mut i));
                assert_eq!(Ty::strong_count(&i), 1);
            }
        });
    }

    #[bench]
    fn bench_insert_and_detach_1(b: &mut Bencher) {
        type Ty = TinyArc<Thread>;
        type CslList = TinyArcList<Thread, OffsetOfCsl>;
        type L = AtomicListHead<Thread, OffsetOfCsl>;
        let n = 1 << 16;
        b.iter(|| {
            let mut head = L::default();
            let mut tail = L::default();
            L::insert_after(&mut head, &mut tail);
            for i in 0..n {
                let mut t = Ty::new(Thread::new(i));
                assert_eq!(Ty::strong_count(&t), 1);
                CslList::insert_before(&mut tail, t.clone());
                assert_eq!(Ty::strong_count(&t), 2);
            }
            for (counter, mut i) in
                TinyArcListIterator::new(&head, Some(NonNull::from_ref(&tail))).enumerate()
            {
                assert_eq!(i.id, counter);
                assert_eq!(Ty::strong_count(&i), 2);
                assert!(CslList::detach(&mut i));
                assert_eq!(Ty::strong_count(&i), 1);
            }
        });
    }
}
