use core::{
    arch::asm,
    sync::atomic::{compiler_fence, Ordering},
};
use cortex_m::interrupt::InterruptNumber;

#[derive(Clone, Copy)]
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

pub fn enable(irq: IrqNumber) {
    unsafe { cortex_m::peripheral::NVIC::unmask(irq) };
}

pub fn disable(irq: IrqNumber) {
    unsafe { cortex_m::peripheral::NVIC::mask(irq) };
}

pub fn is_enabled(irq: IrqNumber) -> bool {
    unsafe { cortex_m::peripheral::NVIC::is_enabled(irq) }
}

pub fn is_active(irq: IrqNumber) -> bool {
    unsafe { cortex_m::peripheral::NVIC::is_active(irq) }
}

#[cfg(armv6m)]
pub const INTERRUPT_TABLE_LEN: usize = 32;
#[cfg(any(armv7m, armv7em))]
pub const INTERRUPT_TABLE_LEN: usize = 240;
#[cfg(armv8m)]
pub const INTERRUPT_TABLE_LEN: usize = 496;

pub type InterruptTable = [Vector; INTERRUPT_TABLE_LEN];
