//! ARM Cortex-M implementation of [`IInterrupt`].
use crate::{cortex_m::Arch, interrupt::IInterrupt};
use core::{
    arch::asm,
    sync::atomic::{compiler_fence, Ordering},
};

impl IInterrupt for Arch {
    fn disable_interrupts() -> usize {
        let r: u32;
        // SAFETY: Safe register read operation
        unsafe { asm!("mrs {}, PRIMASK", out(reg) r, options(nomem, nostack, preserves_flags)) };
        cortex_m::interrupt::disable();
        r as usize
    }

    fn enable_interrupts(state: usize) {
        // Ensure no preceeding memory accesses are reordered to after interrupts are enabled.
        compiler_fence(Ordering::SeqCst);
        // SAFETY: Safe register write operation
        unsafe {
            asm!("msr PRIMASK, {}", in(reg) state);
        }
    }

    fn is_interrupts_active() -> bool {
        let r: u32;
        // SAFETY: Safe register read operation
        unsafe { asm!("mrs {}, PRIMASK", out(reg) r, options(nomem, nostack, preserves_flags)) };
        (r & (1 << 0) != (1 << 0))
    }

    fn is_in_interrupt() -> bool {
        cortex_m::peripheral::SCB::vect_active()
            != cortex_m::peripheral::scb::VectActive::ThreadMode
    }

    fn sys_reset() -> ! {
        cortex_m::peripheral::SCB::sys_reset()
    }
}
