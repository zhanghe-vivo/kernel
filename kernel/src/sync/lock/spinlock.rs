// The MIT License (MIT)
//
// Copyright (c) 2014 Mathijs van de Nes
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.
#![allow(dead_code)]
use crate::{arch::Arch, cpu::Cpu};
#[cfg(debugging_spinlock)]
use crate::{irq::IrqLock, println, thread::Thread};
#[cfg(smp)]
use core::sync::atomic::{AtomicUsize, Ordering};
#[cfg(debugging_spinlock)]
use core::{cell::Cell, ptr::NonNull};
use core::{
    cell::{Cell, UnsafeCell},
    fmt,
    ops::{Deref, DerefMut},
};

#[cfg(smp)]
#[repr(C)]
pub struct Tickets {
    owner: AtomicUsize,
    next: AtomicUsize,
}

#[cfg(smp)]
impl Tickets {
    pub const fn new() -> Self {
        Self {
            owner: AtomicUsize::new(0),
            next: AtomicUsize::new(0),
        }
    }
}

pub struct RawSpin {
    lock: Cell<usize>,
    #[cfg(smp)]
    tickets: Tickets,

    #[cfg(debugging_spinlock)]
    pub(crate) owner: Cell<Option<NonNull<Thread>>>,
}

unsafe impl Sync for RawSpin {}
unsafe impl Send for RawSpin {}

impl RawSpin {
    pub const fn new() -> Self {
        Self {
            lock: Cell::new(0),
            #[cfg(smp)]
            tickets: Tickets::new(),

            #[cfg(debugging_spinlock)]
            owner: Cell::new(None),
        }
    }

    pub fn acquire(&self) -> RawSpinGuard<'_> {
        self.lock();
        RawSpinGuard(self)
    }

    #[inline]
    pub fn lock_fast(&self) {
        #[cfg(debugging_spinlock)]
        if let Some(thread) = crate::current_thread!() {
            let irq_lock = IrqLock::new();
            let _guard = irq_lock.lock();
            let thread = unsafe { thread.as_mut() };
            if thread.check_deadlock(self) {
                unreachable!(
                    "deadlocked, thread {} acquire lock, but is hold by thread {}",
                    thread.get_name(),
                    unsafe { self.owner.get().unwrap().as_ref().get_name() }
                );
            }
            thread.set_wait(self);
        }
        #[cfg(smp)]
        {
            self.arch_lock();
        }
        #[cfg(debugging_spinlock)]
        if let Some(thread) = crate::current_thread!() {
            self.owner.set(Some(thread));
            unsafe { thread.as_mut().clear_wait() };
        }
    }

    #[inline]
    pub fn unlock_fast(&self) {
        #[cfg(debugging_spinlock)]
        {
            self.owner.set(None);
        }
        #[cfg(smp)]
        {
            self.arch_unlock();
        }
    }

    pub fn lock(&self) {
        Cpu::get_current_scheduler().preempt_disable();
        self.lock_fast();
    }

    pub fn unlock(&self) {
        self.unlock_fast();
        Cpu::get_current_scheduler().preempt_enable();
    }

    pub fn lock_irqsave(&self) -> usize {
        #[cfg(smp)]
        {
            Cpu::get_current_scheduler().preempt_disable();
            let level = Arch::disable_interrupts();
            self.lock_fast();
            level
        }

        #[cfg(not(smp))]
        {
            Cpu::get_current_scheduler().preempt_disable();
            Arch::disable_interrupts()
        }
    }

    pub fn unlock_irqrestore(&self, level: usize) {
        #[cfg(smp)]
        {
            self.unlock_fast();
            Arch::enable_interrupts(level);
            Cpu::get_current_scheduler().preempt_enable();
        }

        #[cfg(not(smp))]
        {
            Arch::enable_interrupts(level);
            Cpu::get_current_scheduler().preempt_enable();
        };
    }

    #[cfg(smp)]
    pub fn arch_lock(&self) {
        let lockval = self.tickets.next.fetch_add(1, Ordering::SeqCst);
        while lockval != self.tickets.owner.load(Ordering::SeqCst) {
            Arch::wait_for_event();
        }
    }

    #[cfg(smp)]
    pub fn arch_unlock(&self) {
        self.tickets.owner.fetch_add(1, Ordering::SeqCst);
        Arch::signal_event();
    }
}

pub struct RawSpinGuard<'a>(&'a RawSpin);

impl Drop for RawSpinGuard<'_> {
    #[inline]
    fn drop(&mut self) {
        self.0.unlock();
    }
}

pub struct SpinLock<T: ?Sized> {
    lock: RawSpin,
    data: UnsafeCell<T>,
}

pub struct SpinLockGuard<'a, T: ?Sized + 'a> {
    lock: &'a RawSpin,
    data: &'a mut T,
}

/// A guard that protects some data.
///
/// When the guard is dropped, the next ticket will be processed.
pub struct IrqSpinGuard<'a, T: ?Sized + 'a> {
    irq_save: usize,
    lock: &'a RawSpin,
    data: &'a mut T,
}

unsafe impl<T: ?Sized + Send> Sync for SpinLock<T> {}
unsafe impl<T: ?Sized + Send> Send for SpinLock<T> {}

impl<T> SpinLock<T> {
    /// Creates a new [`IrqSpinLock`] wrapping the supplied data.
    #[inline(always)]
    pub const fn new(data: T) -> Self {
        Self {
            lock: RawSpin::new(),
            data: UnsafeCell::new(data),
        }
    }

    #[inline(always)]
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }

    #[inline(always)]
    pub fn as_mut_ptr(&self) -> *mut T {
        self.data.get()
    }
}

impl<T: ?Sized> SpinLock<T> {
    #[inline(always)]
    pub fn lock(&self) -> SpinLockGuard<T> {
        self.lock.lock();
        SpinLockGuard {
            lock: &self.lock,
            // Safety
            // We know that we are the next ticket to be served,
            // so there's no other thread accessing the data.
            //
            // Every other thread has another ticket number so it's
            // definitely stuck in the spin loop above.
            data: unsafe { &mut *self.data.get() },
        }
    }

    #[inline(always)]
    pub fn lock_irqsave(&self) -> IrqSpinGuard<T> {
        let irq_save = self.lock.lock_irqsave();

        IrqSpinGuard {
            lock: &self.lock,
            irq_save: irq_save,
            // Safety
            // We know that we are the next ticket to be served,
            // so there's no other thread accessing the data.
            //
            // Every other thread has another ticket number so it's
            // definitely stuck in the spin loop above.
            data: unsafe { &mut *self.data.get() },
        }
    }
}

impl<T: ?Sized> SpinLock<T> {
    /// Returns a mutable reference to the underlying data.
    #[inline(always)]
    pub fn get_mut(&mut self) -> &mut T {
        // Safety:
        // We know that there are no other references to `self`,
        // so it's safe to return a exclusive reference to the data.
        unsafe { &mut *self.data.get() }
    }
}

impl<T: ?Sized + Default> Default for SpinLock<T> {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl<T> From<T> for SpinLock<T> {
    fn from(data: T) -> Self {
        Self::new(data)
    }
}

impl<'a, T: ?Sized> SpinLockGuard<'a, T> {
    #[inline(always)]
    pub fn leak(this: Self) -> &'a mut T {
        let data = this.data as *mut _; // Keep it in pointer form temporarily to avoid double-aliasing
        core::mem::forget(this);
        unsafe { &mut *data }
    }
}

impl<'a, T: ?Sized + fmt::Debug> fmt::Debug for SpinLockGuard<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, T: ?Sized + fmt::Display> fmt::Display for SpinLockGuard<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<'a, T: ?Sized> Deref for SpinLockGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.data
    }
}

impl<'a, T: ?Sized> DerefMut for SpinLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.data
    }
}

impl<'a, T: ?Sized> Drop for SpinLockGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.unlock();
    }
}

impl<'a, T: ?Sized> IrqSpinGuard<'a, T> {
    #[inline(always)]
    pub fn leak(this: Self) -> &'a mut T {
        let data = this.data as *mut _;
        core::mem::forget(this);
        unsafe { &mut *data }
    }
}

impl<'a, T: ?Sized + fmt::Debug> fmt::Debug for IrqSpinGuard<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, T: ?Sized + fmt::Display> fmt::Display for IrqSpinGuard<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<'a, T: ?Sized> Deref for IrqSpinGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.data
    }
}

impl<'a, T: ?Sized> DerefMut for IrqSpinGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.data
    }
}

impl<'a, T: ?Sized> Drop for IrqSpinGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.unlock_irqrestore(self.irq_save);
    }
}
