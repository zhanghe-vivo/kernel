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
use crate::cpu::Cpu;
#[cfg(feature = "debugging_spinlock")]
use crate::{irq::IrqLock, println, thread::Thread};
use bluekernel_arch::arch::Arch;
#[cfg(feature = "smp")]
use core::sync::atomic::{AtomicUsize, Ordering};
#[cfg(feature = "debugging_spinlock")]
use core::{cell::Cell, ptr::NonNull};
use core::{
    cell::{Cell, UnsafeCell},
    fmt,
    ops::{Deref, DerefMut},
};

#[cfg(feature = "smp")]
#[repr(C)]
pub struct Tickets {
    owner: AtomicUsize,
    next: AtomicUsize,
}

#[cfg(feature = "smp")]
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
    #[cfg(feature = "smp")]
    tickets: Tickets,

    #[cfg(feature = "debugging_spinlock")]
    pub(crate) owner: Cell<Option<NonNull<Thread>>>,
}

unsafe impl Sync for RawSpin {}
unsafe impl Send for RawSpin {}

impl RawSpin {
    pub const fn new() -> Self {
        Self {
            lock: Cell::new(0),
            #[cfg(feature = "smp")]
            tickets: Tickets::new(),

            #[cfg(feature = "debugging_spinlock")]
            owner: Cell::new(None),
        }
    }

    pub fn acquire(&self) -> RawSpinGuard<'_> {
        self.lock();
        RawSpinGuard(self)
    }

    #[inline]
    pub fn lock_fast(&self) {
        #[cfg(feature = "debugging_spinlock")]
        if let Some(thread) = crate::current_thread!() {
            let irq_lock = IrqLock::new();
            let _guard = irq_lock.lock();
            let thread = unsafe { thread.as_mut() };
            if thread.check_deadlock(self) {
                println!(
                    "deadlocked, thread {} acquire lock, but is hold by thread {}",
                    thread.get_name(),
                    unsafe { self.owner.get().unwrap().as_ref().get_name() }
                );
                assert!(false);
            }
            thread.set_wait(self);
        }
        #[cfg(feature = "smp")]
        {
            self.arch_lock();
        }
        #[cfg(feature = "debugging_spinlock")]
        if let Some(thread) = crate::current_thread!() {
            self.owner.set(Some(thread));
            unsafe { thread.as_mut().clear_wait() };
        }
    }

    #[inline]
    pub fn unlock_fast(&self) {
        #[cfg(feature = "debugging_spinlock")]
        {
            self.owner.set(None);
        }
        #[cfg(feature = "smp")]
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
        #[cfg(feature = "smp")]
        {
            Cpu::get_current_scheduler().preempt_disable();
            let level = Arch::disable_interrupts();
            self.lock_fast();
            level
        }

        #[cfg(not(feature = "smp"))]
        {
            Cpu::get_current_scheduler().preempt_disable();
            Arch::disable_interrupts()
        }
    }

    pub fn unlock_irqrestore(&self, level: usize) {
        #[cfg(feature = "smp")]
        {
            self.unlock_fast();
            Arch::enable_interrupts(level);
            Cpu::get_current_scheduler().preempt_enable();
        }

        #[cfg(not(feature = "smp"))]
        {
            Arch::enable_interrupts(level);
            Cpu::get_current_scheduler().preempt_enable();
        };
    }

    #[cfg(feature = "smp")]
    pub fn arch_lock(&self) {
        let lockval = self.tickets.next.fetch_add(1, Ordering::SeqCst);
        while lockval != self.tickets.owner.load(Ordering::SeqCst) {
            Arch::wait_for_event();
        }
    }

    #[cfg(feature = "smp")]
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

pub struct SpinMutex<T: ?Sized> {
    lock: RawSpin,
    data: UnsafeCell<T>,
}

/// A guard that protects some data.
///
/// When the guard is dropped, the next ticket will be processed.
pub struct SpinMutexGuard<'a, T: ?Sized + 'a> {
    lock: &'a RawSpin,
    data: &'a mut T,
}

unsafe impl<T: ?Sized + Send> Sync for SpinMutex<T> {}
unsafe impl<T: ?Sized + Send> Send for SpinMutex<T> {}

impl<T> SpinMutex<T> {
    /// Creates a new [`SpinMutex`] wrapping the supplied data.
    #[inline(always)]
    pub const fn new(data: T) -> Self {
        Self {
            lock: RawSpin::new(),
            data: UnsafeCell::new(data),
        }
    }

    /// Consumes this [`SpinMutex`] and unwraps the underlying data.
    ///
    /// # Example
    ///
    /// ```
    /// let lock = SpinMutex::<_>::new(42);
    /// assert_eq!(42, lock.into_inner());
    /// ```
    #[inline(always)]
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }
    /// Returns a mutable pointer to the underying data.
    ///
    /// This is mostly meant to be used for applications which require manual unlocking, but where
    /// storing both the lock and the pointer to the inner data gets inefficient.
    ///
    /// # Example
    /// ```
    /// let lock = SpinMutex::<_>::new(42);
    ///
    /// unsafe {
    ///     core::mem::forget(lock.lock());
    ///
    ///     assert_eq!(lock.as_mut_ptr().read(), 42);
    ///     lock.as_mut_ptr().write(58);
    ///
    ///     lock.force_unlock();
    /// }
    ///
    /// assert_eq!(*lock.lock(), 58);
    ///
    /// ```
    #[inline(always)]
    pub fn as_mut_ptr(&self) -> *mut T {
        self.data.get()
    }
}

impl<T: ?Sized> SpinMutex<T> {
    /// Locks the [`SpinMutex`] and returns a guard that permits access to the inner data.
    ///
    /// The returned data may be dereferenced for data access
    /// and the lock will be dropped when the guard falls out of scope.
    ///
    /// ```
    /// let lock = SpinMutex::<_>::new(0);
    /// {
    ///     let mut data = lock.lock();
    ///     // The lock is now locked and the data can be accessed
    ///     *data += 1;
    ///     // The lock is implicitly dropped at the end of the scope
    /// }
    /// ```
    #[inline(always)]
    pub fn lock(&self) -> SpinMutexGuard<T> {
        self.lock.lock();

        SpinMutexGuard {
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
}

impl<T: ?Sized> SpinMutex<T> {
    /// Force unlock this [`SpinMutex`], by serving the next ticket.
    ///
    /// # Safety
    ///
    /// This is *extremely* unsafe if the lock is not held by the current
    /// thread. However, this can be useful in some instances for exposing the
    /// lock to FFI that doesn't know how to deal with RAII.
    #[inline(always)]
    pub unsafe fn force_unlock(&self) {
        self.lock.unlock();
    }

    /// Returns a mutable reference to the underlying data.
    #[inline(always)]
    pub fn get_mut(&mut self) -> &mut T {
        // Safety:
        // We know that there are no other references to `self`,
        // so it's safe to return a exclusive reference to the data.
        unsafe { &mut *self.data.get() }
    }
}

impl<T: ?Sized + Default> Default for SpinMutex<T> {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl<T> From<T> for SpinMutex<T> {
    fn from(data: T) -> Self {
        Self::new(data)
    }
}

impl<'a, T: ?Sized> SpinMutexGuard<'a, T> {
    /// Leak the lock guard, yielding a mutable reference to the underlying data.
    ///
    /// Note that this function will permanently lock the original [`RawSpin`].
    ///
    /// ```
    /// let mylock = spin::mutex::RawSpin::<_>::new(0);
    ///
    /// let data: &mut i32 = spin::mutex::SpinMutexGuard::leak(mylock.lock());
    ///
    /// *data = 1;
    /// assert_eq!(*data, 1);
    /// ```
    #[inline(always)]
    pub fn leak(this: Self) -> &'a mut T {
        let data = this.data as *mut _; // Keep it in pointer form temporarily to avoid double-aliasing
        core::mem::forget(this);
        unsafe { &mut *data }
    }
}

impl<'a, T: ?Sized + fmt::Debug> fmt::Debug for SpinMutexGuard<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, T: ?Sized + fmt::Display> fmt::Display for SpinMutexGuard<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<'a, T: ?Sized> Deref for SpinMutexGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.data
    }
}

impl<'a, T: ?Sized> DerefMut for SpinMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.data
    }
}

impl<'a, T: ?Sized> Drop for SpinMutexGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.unlock();
    }
}

#[macro_export]
macro_rules! new_spinlock {
    ($inner:expr $(, $name:literal)? $(,)?) => {
        $crate::sync::SpinLock::new(
            $inner, $crate::optional_name!($($name)?))
    };
}
pub use new_spinlock;

pub type SpinLock<T> = super::Lock<T, SpinLockBackend>;

pub struct SpinLockBackend;

// SAFETY: The underlying kernel `spinlock_t` object ensures mutual exclusion. `relock` uses the
// default implementation that always calls the same locking method.
unsafe impl super::Backend for SpinLockBackend {
    type State = RawSpin;
    type GuardState = ();

    unsafe fn init(ptr: *mut Self::State, _name: *const core::ffi::c_char) {
        unsafe {
            (*ptr).lock = Cell::new(0);
        }
    }

    unsafe fn lock(ptr: *mut Self::State) -> Self::GuardState {
        unsafe {
            (*ptr).lock.set((*ptr).lock_irqsave());
        }
    }

    unsafe fn unlock(ptr: *mut Self::State, _guard_state: &Self::GuardState) {
        unsafe {
            (*ptr).unlock_irqrestore((*ptr).lock.get());
        }
    }
}
