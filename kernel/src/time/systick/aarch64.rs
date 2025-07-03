use crate::{
    arch::{
        irq::{enable_irq_with_priority, register_handler, IrqHandler, Priority},
        registers::{
            cntfrq_el0::CNTFRQ_EL0, cntp_ctl_el0::CNTP_CTL_EL0, cntp_tval_el0::CNTP_TVAL_EL0,
            cntpct_el0::CNTPCT_EL0,
        },
    },
    boards,
    time::handle_tick_increment,
};
use alloc::boxed::Box;
use spin::Once;
use tock_registers::interfaces::{Readable, Writeable};

pub const SYSTICK_IRQ_NUM: IrqNumber = IrqNumber::new(30);
static BOOT_CYCLE_COUNT: Once<u64> = Once::new();
fn get_boot_cycle_count() -> u64 {
    *BOOT_CYCLE_COUNT.call_once(|| CNTPCT_EL0.get())
}
pub struct SystickIrq {}

impl IrqHandler for SystickIrq {
    fn handle(&mut self) {
        handle_tick_increment();
    }
}

impl Systick {
    pub fn init(&self, _sys_clock: u32, tick_per_second: u32) -> bool {
        register_handler(self.irq_num, Box::new(SystickIrq {}));
        let step = CNTFRQ_EL0.get() / tick_per_second as u64;
        // SAFETY: step is only written once during initialization
        unsafe {
            *self.step.get() = step as usize;
        }
        CNTP_TVAL_EL0.set(step as u64);
        CNTP_CTL_EL0.write(CNTP_CTL_EL0::ENABLE::Enabled);
        for cpu_id in 0..blueos_kconfig::NUM_CORES {
            enable_irq_with_priority(self.irq_num, cpu_id, Priority::Normal);
        }
        let _ = get_boot_cycle_count();
        true
    }

    pub fn get_cycles(&self) -> u64 {
        let current = CNTPCT_EL0.get();
        let boot_cycle_count = get_boot_cycle_count();
        current.saturating_sub(boot_cycle_count)
    }

    pub fn reset_counter(&self) {
        CNTP_TVAL_EL0.set(self.get_step() as u64);
    }
}
