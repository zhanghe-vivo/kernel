use super::sys_config;
use crate::{clock, cpu, error};
use cortex_m::{peripheral::syst::SystClkSource, Peripherals};

#[coverage(off)]
#[no_mangle]
pub extern "C" fn SysTick_Handler() {
    cpu::Cpu::interrupt_nest_inc();

    clock::handle_tick_increase();

    cpu::Cpu::interrupt_nest_dec();
}

pub struct Systick {}

impl Systick {
    pub fn init(tick_per_second: u32) -> Result<(), error::Error> {
        let syst = &mut Peripherals::take().unwrap().SYST;

        let reload = sys_config::SYSTEM_CORE_CLOCK / tick_per_second - 1;
        const SYST_COUNTER_MASK: u32 = 0x00ff_ffff;
        if reload > SYST_COUNTER_MASK {
            return Err(error::code::EINVAL);
        }
        // set SysTick
        syst.set_clock_source(SystClkSource::Core);
        syst.set_reload(reload);
        syst.clear_current();
        // will enbale tick interrupt before start_switch

        Ok(())
    }
}
