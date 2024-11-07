//! ARM Cortex-M implementation of [`ICore`].

use crate::core::ICore;
use cortex_m::peripheral::scb;
use cortex_m::peripheral::scb::VectActive;
use cortex_m::peripheral::syst::SystClkSource;
use cortex_m::{asm, Peripherals};

// only cortex-m used
pub struct ArchCore {
    // cortex_m need to take Peripherals.
    peripherals: Peripherals,
}

impl ICore for ArchCore {
    fn new() -> Self {
        let mut peripherals = unsafe { Peripherals::steal() };
        peripherals.SYST.set_clock_source(SystClkSource::Core);

        ArchCore { peripherals }
    }

    fn start(&mut self) {
        // enable systick
        self.peripherals.SYST.enable_counter();
        self.peripherals.SYST.enable_interrupt();

        // enable PendSV/Systick interrupt on lowest priority
        unsafe {
            self.peripherals
                .SCB
                .set_priority(scb::SystemHandler::PendSV, 0xFF);
            self.peripherals
                .SCB
                .set_priority(scb::SystemHandler::SysTick, 0xFF);
        }
    }
}
