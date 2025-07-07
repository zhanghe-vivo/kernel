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

// We are not using Pin APIs here since Pin APIs are unergonomic and
// hard to learn for ordinary developers. We are using a smart pointer
// to wrap a value, it's conventional that the value is pinned. This
// ListHead should be used with smart pointers. It's **NOT**
// concurrent safe.

use crate::intrusive::Adapter;
use core::{marker::PhantomData, ptr::NonNull};

#[derive(Default, Debug)]
pub struct ListHead<T, A: Adapter> {
    pub prev: Option<NonNull<ListHead<T, A>>>,
    pub next: Option<NonNull<ListHead<T, A>>>,
    _t: PhantomData<T>,
    _a: PhantomData<A>,
}

pub struct ListIterator<T, A: Adapter> {
    next: Option<NonNull<ListHead<T, A>>>,
    tail: Option<NonNull<ListHead<T, A>>>,
    _t: PhantomData<T>,
    _a: PhantomData<A>,
}

impl<T, A: Adapter> ListIterator<T, A> {
    pub fn new(head: &ListHead<T, A>, tail: Option<NonNull<ListHead<T, A>>>) -> Self {
        Self {
            next: head.next,
            tail,
            _t: PhantomData,
            _a: PhantomData,
        }
    }
}

impl<T, A: Adapter> Iterator for ListIterator<T, A> {
    type Item = NonNull<ListHead<T, A>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next == self.tail {
            return None;
        }
        // FIXME: Shall we unwrap_unchecked directly?
        let Some(current) = self.next else {
            panic!("Tail node is specified, but encountered None during iteration");
        };
        self.next = unsafe { current.as_ref().next };
        Some(current)
    }
}

pub struct ListReverseIterator<T, A: Adapter> {
    prev: Option<NonNull<ListHead<T, A>>>,
    head: Option<NonNull<ListHead<T, A>>>,
    _t: PhantomData<T>,
    _a: PhantomData<A>,
}

impl<T, A: Adapter> ListReverseIterator<T, A> {
    pub fn new(tail: &ListHead<T, A>, head: Option<NonNull<ListHead<T, A>>>) -> Self {
        Self {
            prev: tail.prev,
            head,
            _t: PhantomData,
            _a: PhantomData,
        }
    }
}

impl<T, A: Adapter> Iterator for ListReverseIterator<T, A> {
    type Item = NonNull<ListHead<T, A>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.prev == self.head {
            return None;
        }
        // FIXME: Shall we unwrap_unchecked directly?
        let Some(current) = self.prev else {
            panic!("Tail node is specified, but encountered None during iteration");
        };
        self.prev = unsafe { current.as_ref().prev };
        Some(current)
    }
}

impl<T, A: Adapter> ListHead<T, A> {
    pub const fn new() -> Self {
        Self::const_new()
    }

    pub const fn const_new() -> Self {
        Self {
            prev: None,
            next: None,
            _t: PhantomData,
            _a: PhantomData,
        }
    }

    pub fn owner(&self) -> &T {
        let ptr = self as *const _ as *const u8;
        let base = unsafe { ptr.sub(A::offset()) as *const T };
        unsafe { &*base }
    }

    pub unsafe fn owner_mut(&mut self) -> &mut T {
        let ptr = self as *mut _ as *mut u8;
        let base = unsafe { ptr.sub(A::offset()) as *mut T };
        unsafe { &mut *base }
    }

    pub fn is_detached(&self) -> bool {
        self.prev.is_none() && self.next.is_none()
    }

    pub fn insert_after(head: &mut ListHead<T, A>, mut me: NonNull<ListHead<T, A>>) -> bool {
        unsafe {
            if !me.as_ref().is_detached() {
                return false;
            }
            let next = core::mem::replace(&mut head.next, Some(me));
            let _ = core::mem::replace(&mut me.as_mut().next, next);
            let prev = next.map_or(Some(NonNull::from_mut(head)), |mut v| {
                core::mem::replace(&mut v.as_mut().prev, Some(me))
            });
            let _ = core::mem::replace(&mut me.as_mut().prev, prev);
            true
        }
    }

    pub fn insert_before(tail: &mut ListHead<T, A>, mut me: NonNull<ListHead<T, A>>) -> bool {
        unsafe {
            if !me.as_ref().is_detached() {
                return false;
            }
            let prev = core::mem::replace(&mut tail.prev, Some(me));
            let _ = core::mem::replace(&mut me.as_mut().prev, prev);
            let next = prev.map_or(Some(NonNull::from_mut(tail)), |mut v| {
                core::mem::replace(&mut v.as_mut().next, Some(me))
            });
            let _ = core::mem::replace(&mut me.as_mut().next, next);
            true
        }
    }

    pub fn insert_after_with_hook<F: Fn(&ListHead<T, A>)>(
        head: &mut ListHead<T, A>,
        me: NonNull<ListHead<T, A>>,
        hook: F,
    ) -> bool {
        if !Self::insert_after(head, me) {
            return false;
        }
        hook(unsafe { me.as_ref() });
        true
    }

    pub fn detach(mut me: NonNull<ListHead<T, A>>) -> bool {
        unsafe {
            let me_mut = me.as_mut();
            if me_mut.is_detached() {
                return false;
            }
            if let Some(mut prev) = me_mut.prev {
                let _ = core::mem::replace(&mut prev.as_mut().next, me_mut.next);
            };
            if let Some(mut next) = me_mut.next {
                let _ = core::mem::replace(&mut next.as_mut().prev, me_mut.prev);
            };
            me_mut.prev = None;
            me_mut.next = None;
            true
        }
    }

    pub fn detach_with_hook<F>(me: NonNull<ListHead<T, A>>, hook: F) -> bool
    where
        F: Fn(&ListHead<T, A>),
    {
        if !Self::detach(me) {
            return false;
        }
        hook(unsafe { me.as_ref() });
        true
    }
}

impl<T, A> !Send for ListHead<T, A> {}
impl<T, A> !Sync for ListHead<T, A> {}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::offset_of;

    #[derive(Default, Debug)]
    struct OffsetOfLh;

    impl const Adapter for OffsetOfLh {
        #[inline]
        fn offset() -> usize {
            offset_of!(Foo, lh)
        }
    }

    #[derive(Default, Debug)]
    pub struct Foo {
        head: [u8; 8],
        lh: ListHead<Foo, OffsetOfLh>,
        tail: [u8; 8],
    }

    #[test]
    fn test_basic() {
        let f = Foo::default();
        let t = &f.lh;
        let g = t.owner();
        assert_eq!(&f as *const _, g as *const _);
    }

    #[test]
    fn test_insert_and_detach() {
        type Ty = ListHead<Foo, OffsetOfLh>;
        let mut a = Foo::default();
        assert!(a.lh.is_detached());
        let b = Foo::default();
        assert!(b.lh.is_detached());
        assert!(Ty::insert_after(&mut a.lh, NonNull::from_ref(&b.lh)));
        assert!(!a.lh.is_detached());
        assert!(!b.lh.is_detached());
        Ty::detach(NonNull::from_ref(&b.lh));
        assert!(a.lh.is_detached());
        assert!(b.lh.is_detached());
    }
}
