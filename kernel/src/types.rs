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

pub use blueos_infra::{
    impl_simple_intrusive_adapter,
    intrusive::Adapter as IntrusiveAdapter,
    list::typed_ilist::ListHead as IlistHead,
    tinyarc::{
        TinyArc as Arc, TinyArcInner as ArcInner, TinyArcList as ArcList,
        TinyArcListIterator as ArcListIterator,
    },
    tinyrwlock::{IRwLock, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

#[cfg(target_pointer_width = "32")]
mod inner {
    pub type Uint = u8;
    pub type AtomicUint = core::sync::atomic::AtomicU8;
    pub type Int = i8;
    pub type AtomicInt = core::sync::atomic::AtomicI8;
}

#[cfg(target_pointer_width = "64")]
mod inner {
    pub type Uint = usize;
    pub type Int = isize;
    pub type AtomicUint = core::sync::atomic::AtomicUsize;
    pub type AtomicInt = core::sync::atomic::AtomicIsize;
}

#[cfg(target_pointer_width = "64")]
pub type ThreadPriority = u16;
#[cfg(target_pointer_width = "32")]
pub type ThreadPriority = u8;

pub use inner::*;

#[macro_export]
macro_rules! static_arc {
    ($name:ident($ty:ty, $val:expr),) => {
        #[allow(non_snake_case)]
        mod $name {
            use super::*;
            use $crate::types::{Arc, ArcInner};
            static CTRL_BLOCK: ArcInner<$ty> = ArcInner::const_new($val);
            pub(super) static PTR: Arc<$ty> = unsafe { Arc::const_new(&CTRL_BLOCK) };
        }
        use $name::PTR as $name;
    };
}
