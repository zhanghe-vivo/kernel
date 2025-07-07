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

use crate::{
    support::DisableInterruptGuard,
    types::{IRwLock, IntrusiveAdapter, RwLock, RwLockWriteGuard},
};
use core::{
    ops::{Deref, DerefMut},
    sync::atomic::{compiler_fence, Ordering},
};

#[derive(Debug)]
pub struct SpinLock<T: ?Sized> {
    lock: RwLock<T>,
}

// See https://doc.rust-lang.org/reference/destructors.html#r-destructors.operation for dropping orders.
#[derive(Debug)]
#[repr(C)]
pub struct SpinLockGuard<'a, T: ?Sized> {
    mutex_guard: RwLockWriteGuard<'a, T>,
    irq_guard: Option<DisableInterruptGuard>,
}

impl<T: ?Sized> SpinLockGuard<'_, T> {
    #[inline]
    pub fn take_irq_guard<S>(&mut self, other: &mut SpinLockGuard<'_, S>) {
        self.irq_guard = other.irq_guard.take();
    }
}

impl<'a, T: 'a + ?Sized> Deref for SpinLockGuard<'a, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        self.mutex_guard.deref()
    }
}

impl<'a, T: 'a + ?Sized> DerefMut for SpinLockGuard<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        self.mutex_guard.deref_mut()
    }
}

impl<T> SpinLock<T> {
    pub const fn const_new(val: T) -> Self {
        Self {
            lock: RwLock::new(val),
        }
    }

    pub const fn new(val: T) -> Self {
        Self::const_new(val)
    }
}

impl<T: ?Sized> SpinLock<T> {
    pub fn try_irqsave_lock(&self) -> Option<SpinLockGuard<'_, T>> {
        let irq_guard = DisableInterruptGuard::new();
        compiler_fence(Ordering::SeqCst);
        let mut guard = self.try_lock()?;
        assert!(guard.irq_guard.is_none());
        guard.irq_guard = Some(irq_guard);
        Some(guard)
    }

    pub fn irqsave_lock(&self) -> SpinLockGuard<'_, T> {
        loop {
            let Some(l) = self.try_irqsave_lock() else {
                core::hint::spin_loop();
                continue;
            };
            return l;
        }
    }

    pub fn try_lock(&self) -> Option<SpinLockGuard<'_, T>> {
        let mutex_guard = self.lock.try_write()?;
        Some(SpinLockGuard {
            irq_guard: None,
            mutex_guard,
        })
    }

    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        loop {
            let Some(l) = self.try_lock() else {
                core::hint::spin_loop();
                continue;
            };
            return l;
        }
    }
}

impl<'a, T: 'a + ?Sized> SpinLockGuard<'a, T> {
    pub fn forget_irq(&mut self) {
        if self.irq_guard.is_none() {
            return;
        }
        core::mem::forget(self.irq_guard.take())
    }
}

unsafe impl<T: ?Sized + Send> Send for SpinLock<T> {}
unsafe impl<T: ?Sized + Sync> Sync for SpinLock<T> {}

#[derive(Default, Debug)]
pub struct ISpinLock<T: Sized, A: IntrusiveAdapter> {
    lock: IRwLock<T, A>,
}

impl<T: Sized, A: IntrusiveAdapter> ISpinLock<T, A> {
    pub const fn new() -> Self {
        Self {
            lock: IRwLock::new(),
        }
    }

    #[inline]
    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        let l = self.lock.write();
        SpinLockGuard {
            mutex_guard: l,
            irq_guard: None,
        }
    }

    #[inline]
    pub fn irqsave_lock(&self) -> SpinLockGuard<'_, T> {
        let irq_guard = DisableInterruptGuard::new();
        compiler_fence(Ordering::SeqCst);
        let mut g = self.lock();
        g.irq_guard = Some(irq_guard);
        g
    }
}

unsafe impl<T: Sized + Send, A: IntrusiveAdapter> Send for ISpinLock<T, A> {}
unsafe impl<T: Sized + Sync, A: IntrusiveAdapter> Sync for ISpinLock<T, A> {}
