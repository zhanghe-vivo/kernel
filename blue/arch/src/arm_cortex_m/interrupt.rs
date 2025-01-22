//! ARM Cortex-M implementation of [`Interrupt`].
use super::Arch;
use core::{
    arch::asm,
    sync::atomic::{compiler_fence, Ordering},
};
use cortex_m::interrupt::InterruptNumber;

#[doc(hidden)]
#[derive(Copy, Clone)]
#[repr(C)]
pub union Vector {
    pub handler: unsafe extern "C" fn(),
    pub reserved: usize,
}

#[derive(Debug, Copy, Clone)]
#[repr(transparent)]
pub struct IrqNumber(u16);

impl IrqNumber {
    #[inline]
    pub const fn new(number: u16) -> Self {
        Self(number)
    }
}

// SAFETY: get the number of the interrupt is safe
unsafe impl InterruptNumber for IrqNumber {
    #[inline]
    fn number(self) -> u16 {
        self.0
    }
}

impl Arch {
    #[inline]
    pub fn disable_interrupts() -> usize {
        let r: u32;
        // SAFETY: Safe register read operation
        unsafe { asm!("mrs {}, PRIMASK", out(reg) r, options(nomem, nostack, preserves_flags)) };
        cortex_m::interrupt::disable();
        // Impl of cortex_m::interrupt::disable() is
        // ```
        // #[inline]
        // pub fn disable() {
        //     call_asm!(__cpsid());
        // }
        // ```
        // No fence provided.
        // Ensure no preceeding memory accesses are reordered to after interrupts are disabled.
        // This has been fixed in master branch, in https://github.com/rust-embedded/cortex-m/commit/894f2aabdbd65f85eecf25debc2326f0387863c7.
        // But we are sticking to 0.7.7.
        compiler_fence(Ordering::SeqCst);
        r as usize
    }

    #[inline]
    pub fn enable_interrupts(state: usize) {
        // Ensure no preceeding memory accesses are reordered to after interrupts are enabled.
        compiler_fence(Ordering::SeqCst);
        // SAFETY: Safe register write operation
        unsafe {
            asm!("msr PRIMASK, {}", in(reg) state);
        }
    }

    #[inline]
    pub fn is_interrupts_active() -> bool {
        let r: u32;
        // SAFETY: Safe register read operation
        unsafe { asm!("mrs {}, PRIMASK", out(reg) r, options(nomem, nostack, preserves_flags)) };
        (r & (1 << 0) != (1 << 0))
    }

    #[inline]
    pub fn is_in_interrupt() -> bool {
        cortex_m::peripheral::SCB::vect_active()
            != cortex_m::peripheral::scb::VectActive::ThreadMode
    }

    #[inline]
    pub fn sys_reset() -> ! {
        cortex_m::peripheral::SCB::sys_reset()
    }

    #[inline]
    pub fn enable_irq(irq: IrqNumber) {
        unsafe { cortex_m::peripheral::NVIC::unmask(irq) };
    }

    #[inline]
    pub fn disable_irq(irq: IrqNumber) {
        unsafe { cortex_m::peripheral::NVIC::mask(irq) };
    }

    #[inline]
    pub fn is_irq_enabled(irq: IrqNumber) -> bool {
        unsafe { cortex_m::peripheral::NVIC::is_enabled(irq) }
    }

    #[cfg(not(armv6m))]
    #[inline]
    pub fn is_irq_active(irq: IrqNumber) -> bool {
        unsafe { cortex_m::peripheral::NVIC::is_active(irq) }
    }
}
