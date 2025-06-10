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

// IrqNumber to usize
impl From<IrqNumber> for usize {
    fn from(irq: IrqNumber) -> Self {
        usize::from(irq.0)
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

/// Interrupt vector table configuration for ARM Cortex-M processors.
///
/// Users must define their own `__INTERRUPTS` based on their specific device requirements.
/// The interrupt vector table should be placed in the `.vector_table.interrupts` section.
///
/// # Example
///
/// ```rust
/// use core::arch::global_asm;
///
/// #[link_section = ".vector_table.interrupts"]
/// #[no_mangle]
/// static __INTERRUPTS: InterruptTable = [
///     Vector { handler: WWDG },            // Window Watchdog interrupt
///     Vector { handler: PVD },             // PVD through EXTI Line detection
///     Vector { handler: TAMPER },          // Tamper interrupt
///     Vector { handler: RTC },             // RTC global interrupt
///     // ... other device-specific interrupts ...
///     Vector { handler: DEFAULT_HANDLER }, // Default handler for unused interrupts
/// ];
///
/// // Declare external interrupt handlers
/// extern "C" {
///     fn WWDG();
///     fn PVD();
///     fn TAMPER();
///     fn RTC();
/// }
/// ```
///
/// # Architecture-specific Details
///
/// Maximum number of device-specific interrupts for different ARM Cortex-M architectures:
/// - ARMv6-M: 32 interrupts
/// - ARMv7-M/ARMv7E-M: 240 interrupts
/// - ARMv8-M: 496 interrupts
///
/// # Safety
///
/// The interrupt vector table must be properly aligned and contain valid function pointers
/// for all used interrupt vectors. Incorrect configuration may lead to undefined behavior.
#[cfg(armv6m)]
pub const INTERRUPT_TABLE_LEN: usize = 32;
#[cfg(any(armv7m, armv7em))]
pub const INTERRUPT_TABLE_LEN: usize = 240;
#[cfg(armv8m)]
pub const INTERRUPT_TABLE_LEN: usize = 496;

pub type InterruptTable = [Vector; INTERRUPT_TABLE_LEN];
