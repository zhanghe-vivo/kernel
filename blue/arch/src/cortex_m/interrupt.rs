//! ARM Cortex-M implementation of [`Interrupt`].
use crate::cortex_m::Arch;
use core::{
    arch::asm,
    sync::atomic::{compiler_fence, Ordering},
};

impl Arch {
    pub fn disable_interrupts() -> usize {
        let r: u32;
        // SAFETY: Safe register read operation
        unsafe { asm!("mrs {}, PRIMASK", out(reg) r, options(nomem, nostack, preserves_flags)) };
        cortex_m::interrupt::disable();
        r as usize
    }

    pub fn enable_interrupts(state: usize) {
        // Ensure no preceeding memory accesses are reordered to after interrupts are enabled.
        compiler_fence(Ordering::SeqCst);
        // SAFETY: Safe register write operation
        unsafe {
            asm!("msr PRIMASK, {}", in(reg) state);
        }
    }

    pub fn is_interrupts_active() -> bool {
        let r: u32;
        // SAFETY: Safe register read operation
        unsafe { asm!("mrs {}, PRIMASK", out(reg) r, options(nomem, nostack, preserves_flags)) };
        (r & (1 << 0) != (1 << 0))
    }

    pub fn is_in_interrupt() -> bool {
        cortex_m::peripheral::SCB::vect_active()
            != cortex_m::peripheral::scb::VectActive::ThreadMode
    }

    pub fn sys_reset() -> ! {
        cortex_m::peripheral::SCB::sys_reset()
    }
}
