#![allow(dead_code)]
use crate::arch::Arch;
use core::{
    cell::{Cell, UnsafeCell},
    ops::{Deref, DerefMut},
};

use crate::cpu::Cpu;

#[repr(transparent)]
#[derive(Clone, Debug)]
pub struct IrqLockRaw(Cell<usize>);

impl IrqLockRaw {
    #[inline]
    pub const fn new() -> Self {
        Self(Cell::new(0))
    }

    #[inline]
    pub fn lock(&self) -> IrqLockRawGuard<'_> {
        self.raw_lock();
        IrqLockRawGuard(self)
    }

    #[inline]
    fn raw_lock(&self) {
        self.0.replace(Arch::disable_interrupts());
    }

    #[inline]
    fn raw_unlock(&self) {
        Arch::enable_interrupts(self.0.get());
    }
}

pub struct IrqLockRawGuard<'a>(&'a IrqLockRaw);

impl Drop for IrqLockRawGuard<'_> {
    #[inline]
    fn drop(&mut self) {
        self.0.raw_unlock();
    }
}

pub struct IrqLock<T> {
    lock: IrqLockRaw,
    inner: UnsafeCell<T>,
}

impl<T> IrqLock<T> {
    pub const fn new(element: T) -> Self {
        IrqLock {
            lock: IrqLockRaw::new(),
            inner: UnsafeCell::new(element),
        }
    }

    pub fn lock(&self) -> IrqGuard<'_, T> {
        self.raw_lock();
        IrqGuard::new(&self)
    }

    #[inline(always)]
    fn raw_lock(&self) {
        self.lock.raw_lock();
    }

    #[inline(always)]
    fn raw_unlock(&self) {
        self.lock.raw_unlock();
    }
}

unsafe impl<T> Sync for IrqLock<T> {}

pub struct IrqGuard<'a, T> {
    lock: &'a IrqLock<T>,
}

impl<'a, T> IrqGuard<'a, T> {
    fn new(lock: &'a IrqLock<T>) -> Self {
        IrqGuard { lock }
    }
}

impl<'a, T> Deref for IrqGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.inner.get() }
    }
}

impl<'a, T> DerefMut for IrqGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.inner.get() }
    }
}

impl<'a, T> Drop for IrqGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.raw_unlock();
    }
}

pub struct Irq;

impl Irq {
    pub fn enter() {
        Cpu::interrupt_nest_inc();
    }

    pub fn leave() {
        Cpu::interrupt_nest_dec();
    }
}
