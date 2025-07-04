// Copyright (c) 2025 vivo Mobile Communication Co., Ltd.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//       http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::arch::irq::IRQ_PRIORITY_FOR_SCHEDULER;
use cortex_m::{
    peripheral::{scb::SystemHandler, syst::SystClkSource, SYST},
    Peripherals,
};

pub const SYSTICK_IRQ_NUM: IrqNumber = IrqNumber::new(14);

impl Systick {
    pub fn init(&self, sys_clock: u32, tick_per_second: u32) -> bool {
        let mut scb = unsafe { Peripherals::steal() };

        let reload = sys_clock / tick_per_second;
        const SYST_COUNTER_MASK: u32 = 0x00ff_ffff;
        if reload > SYST_COUNTER_MASK {
            return false;
        }
        // SAFETY: step is only written once during initialization
        unsafe {
            *self.step.get() = reload as usize;
        }
        // set SysTick
        unsafe {
            scb.SCB
                .set_priority(SystemHandler::SysTick, IRQ_PRIORITY_FOR_SCHEDULER);
        }
        scb.SYST.set_clock_source(SystClkSource::Core);
        scb.SYST.set_reload(reload);
        scb.SYST.clear_current();
        scb.SYST.enable_counter();
        scb.SYST.enable_interrupt();
        true
    }

    pub fn get_cycles(&self) -> u64 {
        let step = self.get_step() as u64;
        let current = step - SYST::get_current() as u64;
        let ticks = self.get_tick() as u64;
        ticks * step + current
    }

    pub fn reset_counter(&self) {
        // no need to reset counter
    }
}
