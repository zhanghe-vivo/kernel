use crate::rt_bindings::*;
use core::ptr::addr_of_mut;
use core::ptr::read_volatile;
use core::ptr::write_volatile;

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

#[cfg(not(feature = "RT_USING_SMP"))]
#[no_mangle]
pub static mut rt_interrupt_nest: rt_uint8_t = 0;

#[inline]
fn interrupt_nest_addr_mut() -> *mut rt_uint8_t {
    unsafe {
        #[cfg(feature = "RT_USING_SMP")]
        return addr_of_mut!((*rt_cpu_self()).irq_nest) as *mut rt_uint8_t;

        #[cfg(not(feature = "RT_USING_SMP"))]
        return addr_of_mut!(rt_interrupt_nest);
    }
}

#[inline]
unsafe fn interrupt_nest_get() -> rt_uint8_t {
    return read_volatile(interrupt_nest_addr_mut());
}
#[inline]
unsafe fn interrupt_nest_set(num: rt_uint8_t) {
    write_volatile(interrupt_nest_addr_mut(), num);
}

/// This function will be invoked by BSP, when entering interrupt service routine
///
/// Note: Please don't invoke this routine in application
///
/// See `rt_interrupt_leave`
#[linkage = "weak"]
#[no_mangle]
pub unsafe extern "C" fn rt_interrupt_enter() {
    let level = rt_hw_interrupt_disable();
    let nest = interrupt_nest_get() + 1;
    interrupt_nest_set(nest);
    crate::rt_object_hook_call!(rt_interrupt_enter_hook);
    rt_hw_interrupt_enable(level);
}

/// This function will be invoked by BSP, when leaving interrupt service routine
///
/// Note: Please don't invoke this routine in application
///
/// See `rt_interrupt_enter`
#[linkage = "weak"]
#[no_mangle]
pub unsafe extern "C" fn rt_interrupt_leave() {
    let level = rt_hw_interrupt_disable();
    crate::rt_object_hook_call!(rt_interrupt_leave_hook);
    let nest = interrupt_nest_get() - 1;
    interrupt_nest_set(nest);
    rt_hw_interrupt_enable(level);
}

/// This function will return the nest of interrupt.
///
/// User application can invoke this function to get whether the current
/// context is an interrupt context.
///
/// Returns the number of nested interrupts.
#[linkage = "weak"]
#[no_mangle]
pub unsafe extern "C" fn rt_interrupt_get_nest() -> rt_uint8_t {
    let level = rt_hw_interrupt_disable();
    let ret = interrupt_nest_get();
    rt_hw_interrupt_enable(level);
    ret
}

#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn rt_hw_interrupt_is_disabled() -> bool {
    false
}
