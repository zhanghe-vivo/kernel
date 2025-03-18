#[cfg(feature = "smp")]
use crate::arch::Arch;
use crate::kernel::cpu::Cpu;

#[no_mangle]
pub unsafe extern "C" fn rt_interrupt_nest_load() -> u32 {
    Cpu::interrupt_nest_load()
}

#[no_mangle]
pub unsafe extern "C" fn rt_interrupt_nest_inc() -> u32 {
    Cpu::interrupt_nest_inc()
}

#[no_mangle]
pub unsafe extern "C" fn rt_interrupt_nest_dec() -> u32 {
    Cpu::interrupt_nest_dec()
}

/// This function will lock all cpus's scheduler and disable local irq.
/// Return current cpu interrupt status.
#[cfg(feature = "smp")]
#[no_mangle]
pub unsafe extern "C" fn rt_cpus_lock() -> usize {
    let level = Arch::disable_interrupts();
    Cpus::lock_cpus();
    level
}

/// This function will restore all cpus's scheduler and restore local irq.
/// level is interrupt status returned by rt_cpus_lock().
#[cfg(feature = "smp")]
#[no_mangle]
pub unsafe extern "C" fn rt_cpus_unlock(level: usize) {
    Cpus::unlock_cpus();
    Arch::enable_interrupts(level);
}
