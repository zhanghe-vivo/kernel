use crate::kernel::{cpu::Cpu, irq};
/// This function will be invoked by BSP, when entering interrupt service routine
///
/// Note: Please don't invoke this routine in application
///
/// See `rt_interrupt_leave`
#[linkage = "weak"]
#[no_mangle]
pub unsafe extern "C" fn rt_interrupt_enter() {
    let lock = irq::IrqLockRaw::new();
    let _guard = lock.lock();

    Cpu::interrupt_nest_inc();
}

/// This function will be invoked by BSP, when leaving interrupt service routine
///
/// Note: Please don't invoke this routine in application
///
/// See `rt_interrupt_enter`
#[linkage = "weak"]
#[no_mangle]
pub unsafe extern "C" fn rt_interrupt_leave() {
    let lock = irq::IrqLockRaw::new();
    let _guard = lock.lock();
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
pub extern "C" fn rt_interrupt_get_nest() -> u32 {
    Cpu::interrupt_nest_load()
}

#[linkage = "weak"]
#[no_mangle]
pub extern "C" fn rt_hw_interrupt_is_disabled() -> bool {
    false
}
