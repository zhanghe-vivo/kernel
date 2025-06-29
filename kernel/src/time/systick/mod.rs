use crate::{arch::irq::IrqNumber, config, scheduler, time::timer};
use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicUsize, Ordering},
};

#[cfg(cortex_m)]
include!("cortex_m.rs");
#[cfg(target_arch = "aarch64")]
include!("aarch64.rs");
#[cfg(target_arch = "riscv64")]
include!("riscv64.rs");

pub(crate) static SYSTICK: Systick = Systick::new(SYSTICK_IRQ_NUM);

pub struct Systick {
    tick: AtomicUsize,
    irq_num: IrqNumber,
    step: UnsafeCell<usize>,
}

// SAFETY: step is only written once during initialization and then only read
unsafe impl Sync for Systick {}

impl Systick {
    pub const fn new(irq_num: IrqNumber) -> Self {
        Self {
            irq_num,
            tick: AtomicUsize::new(0),
            step: UnsafeCell::new(0),
        }
    }

    pub fn irq_num(&self) -> IrqNumber {
        self.irq_num
    }

    pub fn get_step(&self) -> usize {
        // SAFETY: step is only read after initialization
        unsafe { *self.step.get() }
    }
}
