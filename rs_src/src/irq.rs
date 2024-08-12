#![allow(dead_code)]
use crate::{cpu::Cpu, rt_bindings};
use core::cell::Cell;

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
pub struct IrqLock(Cell<rt_bindings::rt_base_t>);

impl IrqLock {
    #[inline]
    pub const fn new() -> Self {
        Self(Cell::new(0))
    }

    #[inline]
    pub fn lock(&self) -> IrqLockGuard<'_> {
        unsafe { self.0.replace(rt_bindings::rt_hw_local_irq_disable()) };
        IrqLockGuard(self)
    }

    #[inline]
    pub fn unlock(&self) {
        unsafe { rt_bindings::rt_hw_local_irq_enable(self.0.get()) };
    }
}

pub struct IrqLockGuard<'a>(&'a IrqLock);

impl Drop for IrqLockGuard<'_> {
    #[inline]
    fn drop(&mut self) {
        self.0.unlock();
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
    let lock = IrqLock::new();
    let _guard = lock.lock();

    Cpu::interrupt_nest_inc();
    crate::rt_object_hook_call!(rt_interrupt_enter_hook);
}

/// This function will be invoked by BSP, when leaving interrupt service routine
///
/// Note: Please don't invoke this routine in application
///
/// See `rt_interrupt_enter`
#[linkage = "weak"]
#[no_mangle]
pub unsafe extern "C" fn rt_interrupt_leave() {
    let lock = IrqLock::new();
    let _guard = lock.lock();
    crate::rt_object_hook_call!(rt_interrupt_leave_hook);
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
pub unsafe extern "C" fn rt_interrupt_get_nest() -> rt_bindings::rt_uint32_t {
    Cpu::interrupt_nest_load()
}

#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn rt_hw_interrupt_is_disabled() -> bool {
    false
}
