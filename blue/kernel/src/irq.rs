#![allow(dead_code)]
use crate::cpu::Cpu;
use blue_arch::arch::Arch;
use blue_arch::IInterrupt;
use core::cell::{Cell, UnsafeCell};
use core::ops::{Deref, DerefMut};
use rt_bindings;

#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
type IrqHookFn = unsafe extern "C" fn();

#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
#[no_mangle]
pub static mut rt_interrupt_enter_hook: Option<IrqHookFn> = None;

#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
#[no_mangle]
pub static mut rt_interrupt_leave_hook: Option<IrqHookFn> = None;

#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
#[no_mangle]
pub unsafe extern "C" fn rt_interrupt_enter_sethook(hook: unsafe extern "C" fn()) {
    rt_interrupt_enter_hook = Some(hook);
}

#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
#[no_mangle]
pub unsafe extern "C" fn rt_interrupt_leave_sethook(hook: unsafe extern "C" fn()) {
    rt_interrupt_leave_hook = Some(hook);
}

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

/// This function will be invoked by BSP, when entering interrupt service routine
///
/// Note: Please don't invoke this routine in application
///
/// See `rt_interrupt_leave`
#[linkage = "weak"]
#[no_mangle]
pub unsafe extern "C" fn rt_interrupt_enter() {
    let lock = IrqLockRaw::new();
    let _guard = lock.lock();

    Cpu::interrupt_nest_inc();
    rt_bindings::rt_object_hook_call!(rt_interrupt_enter_hook);
}

/// This function will be invoked by BSP, when leaving interrupt service routine
///
/// Note: Please don't invoke this routine in application
///
/// See `rt_interrupt_enter`
#[linkage = "weak"]
#[no_mangle]
pub unsafe extern "C" fn rt_interrupt_leave() {
    let lock = IrqLockRaw::new();
    let _guard = lock.lock();
    rt_bindings::rt_object_hook_call!(rt_interrupt_leave_hook);
    Cpu::interrupt_nest_dec();
}

/// This function will return the nest of interrupt.
///
/// User application can invoke this function to get whether the current
/// context is an interrupt context.
///
/// Returns the number of nested interrupts.
#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn rt_interrupt_get_nest() -> rt_bindings::rt_uint32_t {
    Cpu::interrupt_nest_load()
}

#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn rt_hw_interrupt_is_disabled() -> bool {
    false
}
