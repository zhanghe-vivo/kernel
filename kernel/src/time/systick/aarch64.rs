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
        let cpu_id = arch::current_cpu_id();
        if cpu_id == 0 {
            register_handler(self.irq_num, Box::new(SystickIrq {}));
            let _ = get_boot_cycle_count();
        }

        let step = CNTFRQ_EL0.get() / tick_per_second as u64;
        // SAFETY: step is only written once during initialization
        unsafe {
            *self.step.get() = step as usize;
        }
        CNTP_TVAL_EL0.set(step as u64);
        CNTP_CTL_EL0.write(CNTP_CTL_EL0::ENABLE::Enabled);
        enable_irq_with_priority(self.irq_num, cpu_id, Priority::Normal);
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
