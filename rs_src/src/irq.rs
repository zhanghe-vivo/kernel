use crate::cpu::Cpu;
use crate::rt_bindings::{self, *};

#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
#[no_mangle]
pub static mut rt_interrupt_enter_hook: Option<unsafe extern "C" fn()> = None;

#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
#[no_mangle]
pub static mut rt_interrupt_leave_hook: Option<unsafe extern "C" fn()> = None;

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

struct IrqLock {
    level: rt_bindings::rt_base_t,
}

impl IrqLock {
    #[inline]
    pub const fn new() -> Self {
        Self { level: 0 }
    }

    #[inline]
    pub fn lock(&mut self) -> IrqLockGuard<'_> {
        self.level = unsafe { rt_bindings::rt_hw_local_irq_disable() };
        IrqLockGuard(self)
    }

    #[inline]
    pub fn unlock(&self) {
        unsafe { rt_bindings::rt_hw_local_irq_enable(self.level) };
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
pub unsafe extern "C" fn rt_interrupt_get_nest() -> rt_uint32_t {
    Cpu::interrupt_nest_load()
}

#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn rt_hw_interrupt_is_disabled() -> bool {
    false
}
