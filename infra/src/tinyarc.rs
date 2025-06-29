#![allow(unused)]
extern crate alloc;
use crate::{
    intrusive::Adapter,
    list::typed_ilist::{ListHead, ListIterator, ListReverseIterator},
};
use alloc::boxed::Box;
use core::{
    marker::PhantomData,
    ops::Deref,
    ptr::NonNull,
    sync::atomic::{fence, Ordering},
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
pub struct TinyArcInner<T: Sized> {
    data: T,
    // We don't need a large counter as Arc, we don't have weak
    // counter either.
    rc: AtomicUint,
}

impl<T: Sized> TinyArcInner<T> {
    pub const fn const_new(data: T) -> Self {
        Self {
            data: data,
            rc: AtomicUint::new(1),
        }
    }

    pub const fn new(data: T) -> Self {
        Self::const_new(data)
    }
}

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

    pub unsafe fn get_handle(this: &Self) -> *const u8 {
        this.inner.as_ref() as *const _ as *const u8
    }

    pub fn strong_count(this: &Self) -> usize {
        unsafe { this.inner.as_ref().rc.load(Ordering::Relaxed) as usize }
    }

    pub unsafe fn increment_strong_count(this: &Self) {
        let old = this.inner.as_ref().rc.fetch_add(1, Ordering::Relaxed);
        assert_ne!(old, 0);
    }

    pub unsafe fn decrement_strong_count(this: &Self) {
        let old = this.inner.as_ref().rc.fetch_sub(1, Ordering::Relaxed);
        assert_ne!(old, 1);
    }

    pub fn is(&self, other: &Self) -> bool {
        unsafe { Self::get_handle(self) == Self::get_handle(other) }
    }
}

impl<T: Sized> Clone for TinyArc<T> {
    #[inline]
    fn clone(&self) -> TinyArc<T> {
        let old = unsafe { self.inner.as_ref() }
            .rc
            .fetch_add(1, Ordering::Relaxed);
        assert!(old >= 1);
        return TinyArc {
            inner: self.inner.clone(),
        };
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

// TinyArc doesn't contain the value it manages, but a pointer to the
// value. So it's safe to impl Send + Sync for it.
unsafe impl<T: Sized> Send for TinyArc<T> {}
unsafe impl<T: Sized> Sync for TinyArc<T> {}

// This list is semi-safe for concurrency. Following usage is
// considered safe if a node might be inserted to several lists:
// Acquire the lock of the list and then the lock of the node, after
// that insert the node or detach the node.
//
// It's **UNSAFE** to detach a node directly if the node might be
// inserted to multiple lists.
#[derive(Default, Debug)]
pub struct TinyArcList<T: Sized, A: Adapter> {
    len: usize,
    head: ListHead<T, A>,
    tail: ListHead<T, A>,
    _t: PhantomData<T>,
    _a: PhantomData<A>,
}

impl<T: Sized, A: Adapter> TinyArcList<T, A> {
    pub const fn const_new() -> Self {
        Self {
            len: 0,
            head: ListHead::<T, A>::const_new(),
            tail: ListHead::<T, A>::const_new(),
            _t: PhantomData,
            _a: PhantomData,
        }
    }

    pub const fn new() -> Self {
        Self::const_new()
    }

    #[inline]
    pub fn init(&mut self) -> bool {
        return ListHead::<T, A>::insert_after(&mut self.head, NonNull::from_mut(&mut self.tail));
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        assert_eq!(
            self.head.next == Some(NonNull::from_ref(&self.tail)),
            self.len == 0
        );
        return self.head.next == Some(NonNull::from_ref(&self.tail));
    }

    #[inline]
    fn list_head_of(this: &TinyArc<T>) -> NonNull<ListHead<T, A>> {
        let other_val = this.deref();
        let ptr = other_val as *const _ as *const u8;
        let list_head_ptr = unsafe { ptr.add(A::offset()) as *const ListHead<T, A> };
        return NonNull::from_ref(unsafe { &*list_head_ptr });
    }

    #[inline]
    pub unsafe fn list_head_of_mut(this: &TinyArc<T>) -> &mut ListHead<T, A> {
        let other_val = this.deref();
        let ptr = other_val as *const _ as *const u8;
        let list_head_ptr = ptr.add(A::offset()) as *mut ListHead<T, A>;
        &mut *list_head_ptr
    }

    #[inline]
    pub unsafe fn make_arc_from(node: &ListHead<T, A>) -> TinyArc<T> {
        let ptr = node as *const _ as *const u8;
        let mut offset = core::mem::offset_of!(TinyArcInner<T>, data);
        offset += A::offset();
        unsafe {
            let inner = &*(ptr.sub(offset) as *const TinyArcInner<T>);
            return TinyArc::from_inner(NonNull::from_ref(inner));
        }
    }

    pub fn insert_after(other_node: &mut ListHead<T, A>, me: TinyArc<T>) -> bool {
        let me_node = Self::list_head_of(&me);
        if !ListHead::<T, A>::insert_after(other_node, me_node) {
            return false;
        }
        // The list shares ownership of me.
        unsafe { TinyArc::<T>::increment_strong_count(&me) };
        return true;
    }

    pub fn insert_before(other_node: &mut ListHead<T, A>, me: TinyArc<T>) -> bool {
        let me_node = Self::list_head_of(&me);
        if !ListHead::<T, A>::insert_before(other_node, me_node) {
            return false;
        }
        // The list shares ownership of me.
        unsafe { TinyArc::<T>::increment_strong_count(&me) };
        return true;
    }

    pub fn push_back(&mut self, me: TinyArc<T>) -> bool {
        if Self::insert_before(&mut self.tail, me) {
            self.len += 1;
            return true;
        }
        return false;
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn pop_front(&mut self) -> Option<TinyArc<T>> {
        assert!(self.head.next.is_some());
        if self.is_empty() {
            return None;
        }
        let Some(next) = self.head.next else {
            panic!("Head's next node should not be None");
        };
        let arc = unsafe { Self::make_arc_from(next.as_ref()) };
        let ok = ListHead::<T, A>::detach(next);
        assert!(ok);
        unsafe { TinyArc::<T>::decrement_strong_count(&arc) };
        self.len -= 1;
        return Some(arc);
    }

    pub fn detach(me: &TinyArc<T>) -> bool {
        let me_node = Self::list_head_of(me);
        if !ListHead::<T, A>::detach(me_node) {
            return false;
        }
        unsafe { TinyArc::<T>::decrement_strong_count(me) };
        return true;
    }

    pub fn clear(&mut self) -> usize {
        let mut c = 0;
        for i in TinyArcListIterator::<T, A>::new(&self.head, Some(NonNull::from_ref(&self.tail))) {
            Self::detach(&i);
            c += 1;
        }
        self.len = 0;
        return c;
    }

    pub fn iter(&self) -> TinyArcListIterator<T, A> {
        TinyArcListIterator::<T, A>::new(&self.head, Some(NonNull::from_ref(&self.tail)))
    }
}

impl<T: Sized, A: Adapter> Drop for TinyArcList<T, A> {
    #[inline]
    fn drop(&mut self) {
        // FIXME: Warn if self.len != 0 since memory might leak.
        // NOTE: Elements should be cleared by calling `clear` method
        // since move occurs when dropping. Do you recall how drop is
        // called? It's `drop(val)`.
    }
}

pub struct TinyArcListIterator<T, A: Adapter> {
    it: ListIterator<T, A>,
}

pub struct TinyArcListReverseIterator<T, A: Adapter> {
    it: ListReverseIterator<T, A>,
}

impl<T, A: Adapter> TinyArcListIterator<T, A> {
    pub fn new(head: &ListHead<T, A>, tail: Option<NonNull<ListHead<T, A>>>) -> Self {
        Self {
            it: ListIterator::new(head, tail),
        }
    }
}

impl<T, A: Adapter> TinyArcListReverseIterator<T, A> {
    pub fn new(tail: &ListHead<T, A>, head: Option<NonNull<ListHead<T, A>>>) -> Self {
        Self {
            it: ListReverseIterator::new(tail, head),
        }
    }
}

impl<T, A: Adapter> Iterator for TinyArcListIterator<T, A> {
    type Item = TinyArc<T>;

    fn next(&mut self) -> Option<Self::Item> {
        let Some(node) = self.it.next() else {
            return None;
        };
        return Some(unsafe { TinyArcList::<T, A>::make_arc_from(node.as_ref()) });
    }
}

impl<T, A: Adapter> Iterator for TinyArcListReverseIterator<T, A> {
    type Item = TinyArc<T>;

    fn next(&mut self) -> Option<Self::Item> {
        let Some(node) = self.it.next() else {
            return None;
        };
        return Some(unsafe { TinyArcList::<T, A>::make_arc_from(node.as_ref()) });
    }
}

#[cfg(test)]
mod tests {
    extern crate test;
    use super::*;
    use crate::{impl_simple_intrusive_adapter, list::typed_ilist::ListHead, tinyrwlock::RwLock};
    use test::Bencher;

    impl_simple_intrusive_adapter!(OffsetOfCsl, Thread, control_status_list);
    impl_simple_intrusive_adapter!(OffsetOfTl, Thread, timer_list);

    #[derive(Default, Debug)]
    pub struct Thread {
        pub control_status_list: ListHead<Thread, OffsetOfCsl>,
        pub timer_list: ListHead<Thread, OffsetOfTl>,
        pub id: usize,
    }

    impl Thread {
        pub fn new(id: usize) -> Self {
            let mut res = Self::default();
            res.id = id;
            return res;
        }
    }

    #[test]
    fn test_basic() {
        type Ty = TinyArc<Thread>;
        type CslList = TinyArcList<Thread, OffsetOfCsl>;
        type TlList = TinyArcList<Thread, OffsetOfTl>;
        let t = TinyArc::new(Thread::default());
        assert_eq!(
            &t.control_status_list as *const _,
            CslList::list_head_of(&t).as_ptr()
        );
        assert_eq!(&t.timer_list as *const _, TlList::list_head_of(&t).as_ptr());
    }

    #[test]
    fn test_detach_during_iter() {
        type Ty = TinyArc<Thread>;
        type CslList = TinyArcList<Thread, OffsetOfCsl>;
        type L = ListHead<Thread, OffsetOfCsl>;
        let mut head = RwLock::new(L::default());
        let mut w = head.write();
        let t = TinyArc::new(Thread::default());
        CslList::insert_after(&mut *w, t);
        for e in TinyArcListIterator::new(&*w, None) {
            CslList::detach(&e);
        }
    }

    #[test]
    fn test_insert_and_detach() {
        type Ty = TinyArc<Thread>;
        type CslList = TinyArcList<Thread, OffsetOfCsl>;
        type L = ListHead<Thread, OffsetOfCsl>;
        let n = 4;
        let mut head = L::default();
        for i in 0..n {
            let t = Ty::new(Thread::new(i));
            assert_eq!(Ty::strong_count(&t), 1);
            CslList::insert_after(&mut head, t.clone());
            assert_eq!(Ty::strong_count(&t), 2);
        }
        let mut counter = (n - 1) as isize;
        for i in TinyArcListIterator::new(&head, None) {
            assert_eq!(i.id, counter as usize);
            assert_eq!(Ty::strong_count(&i), 2);
            counter -= 1;
            assert!(CslList::detach(&i));
            assert_eq!(Ty::strong_count(&i), 1);
        }
    }

    #[test]
    fn test_insert_before_and_detach() {
        type Ty = TinyArc<Thread>;
        type CslList = TinyArcList<Thread, OffsetOfCsl>;
        type L = ListHead<Thread, OffsetOfCsl>;
        let n = 4;
        let mut tail = L::default();
        for i in 0..n {
            let t = Ty::new(Thread::new(i));
            assert_eq!(Ty::strong_count(&t), 1);
            CslList::insert_before(&mut tail, t.clone());
            assert_eq!(Ty::strong_count(&t), 2);
        }
        let mut counter = (n - 1) as isize;
        for i in TinyArcListReverseIterator::new(&tail, None) {
            assert_eq!(i.id, counter as usize);
            assert_eq!(Ty::strong_count(&i), 2);
            counter -= 1;
            assert!(CslList::detach(&i));
            assert_eq!(Ty::strong_count(&i), 1);
        }
    }

    #[test]
    fn test_push_and_pop() {
        type Ty = TinyArc<Thread>;
        type CslList = TinyArcList<Thread, OffsetOfCsl>;
        type L = ListHead<Thread, OffsetOfCsl>;
        let n = 16;
        let mut l = CslList::default();
        l.init();
        for i in 0..n {
            let t = Ty::new(Thread::new(i));
            assert_eq!(Ty::strong_count(&t), 1);
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
        type L = ListHead<Thread, OffsetOfCsl>;
        let n = 16;
        let mut l = CslList::default();
        l.init();
        for i in 0..n {
            let t = Ty::new(Thread::new(i));
            assert_eq!(Ty::strong_count(&t), 1);
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
        type L = ListHead<Thread, OffsetOfCsl>;
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
            if let Some(t) = iter.next() {
                assert_eq!(Ty::strong_count(&t), 2);
                assert!(CslList::detach(&t));
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
        type L = ListHead<Thread, OffsetOfCsl>;
        let mut l = CslList::default();
        l.init();
        let n = 16;
        for i in 0..n {
            let t = Ty::new(Thread::new(i));
            assert_eq!(Ty::strong_count(&t), 1);
            assert!(l.push_back(t.clone()));
        }

        for i in 0..n {
            let mut iter = l.iter();
            if let Some(t) = iter.next() {
                assert_eq!(Ty::strong_count(&t), 2);
                assert!(CslList::detach(&t));
                assert_eq!(Ty::strong_count(&t), 1);
                // insert back to the list again
                l.push_back(t.clone());
            }
        }
        l.clear();
    }

    #[bench]
    fn bench_insert_and_detach(b: &mut Bencher) {
        type Ty = TinyArc<Thread>;
        type CslList = TinyArcList<Thread, OffsetOfCsl>;
        type L = ListHead<Thread, OffsetOfCsl>;
        let n = 1 << 16;
        b.iter(|| {
            let mut head = L::default();
            let tail = L::default();
            L::insert_after(&mut head, NonNull::from_ref(&tail));
            for i in 0..n {
                let t = Ty::new(Thread::new(i));
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
        type L = ListHead<Thread, OffsetOfCsl>;
        let n = 1 << 16;
        b.iter(|| {
            let mut head = L::default();
            let mut tail = L::default();
            L::insert_after(&mut head, NonNull::from_ref(&tail));
            for i in 0..n {
                let t = Ty::new(Thread::new(i));
                assert_eq!(Ty::strong_count(&t), 1);
                CslList::insert_before(&mut tail, t.clone());
                assert_eq!(Ty::strong_count(&t), 2);
            }
            let mut counter = 0;
            for mut i in TinyArcListIterator::new(&head, Some(NonNull::from_ref(&tail))) {
                assert_eq!(i.id, counter);
                assert_eq!(Ty::strong_count(&i), 2);
                counter += 1;
                assert!(CslList::detach(&mut i));
                assert_eq!(Ty::strong_count(&i), 1);
            }
        });
    }
}
