use super::sys_config::{TICK_PER_SECOND, TIME_IRQ_NUM};
use crate::{
    arch::{
        interrupt::IrqHandler,
        registers::{
            cntfrq_el0::CNTFRQ_EL0, cntp_ctl_el0::CNTP_CTL_EL0, cntp_tval_el0::CNTP_TVAL_EL0,
        },
        Arch,
    },
    clock, error,
    irq::Irq,
};
// use bluekernel_kconfig::CPUS_NR;
use alloc::boxed::Box;
use tock_registers::interfaces::{Readable, Writeable};
pub struct Systick {}

pub struct SystickIrq {}

static mut STEP: u64 = 0;

impl IrqHandler for SystickIrq {
    fn handle(&mut self) -> Result<(), &'static str> {
        Irq::enter();
        clock::handle_tick_increase();
        unsafe { CNTP_TVAL_EL0.set(STEP) };
        Irq::leave();
        Ok(())
    }
}

impl Systick {
    pub fn init() -> Result<(), error::Error> {
        Arch::register_handler(TIME_IRQ_NUM, Box::new(SystickIrq {}));
        unsafe {
            STEP = CNTFRQ_EL0.get() / TICK_PER_SECOND;
            CNTP_TVAL_EL0.set(STEP as u64);
        }
        Arch::enable_irq(TIME_IRQ_NUM, 0);
        CNTP_CTL_EL0.write(CNTP_CTL_EL0::ENABLE::Enabled);
        Ok(())
    }
}
