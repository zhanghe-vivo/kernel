use crate::arch::Arch;

#[no_mangle]
#[inline(always)]
pub extern "C" fn __rt_ffs(value: core::ffi::c_int) -> core::ffi::c_int {
    (value.trailing_zeros() + 1) as core::ffi::c_int
}

#[no_mangle]
#[inline(always)]
pub extern "C" fn rt_hw_local_irq_disable() -> core::ffi::c_long {
    Arch::disable_interrupts() as core::ffi::c_long
}

#[no_mangle]
#[inline(always)]
pub extern "C" fn rt_hw_local_irq_enable(level: core::ffi::c_long) {
    Arch::enable_interrupts(level as usize);
}

#[cfg(not(feature = "smp"))]
#[no_mangle]
#[inline(always)]
pub extern "C" fn rt_hw_interrupt_disable() -> core::ffi::c_long {
    rt_hw_local_irq_disable() as core::ffi::c_long
}

#[cfg(not(feature = "smp"))]
#[no_mangle]
#[inline(always)]
pub extern "C" fn rt_hw_interrupt_enable(level: core::ffi::c_long) {
    rt_hw_local_irq_enable(level);
}

#[no_mangle]
pub extern "C" fn rt_hw_cpu_id() -> core::ffi::c_int {
    Arch::core_id()
}
