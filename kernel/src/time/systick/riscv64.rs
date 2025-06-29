use crate::{arch, boards};

pub const SYSTICK_IRQ_NUM: IrqNumber = IrqNumber::new(arch::TIMER_INT);

impl Systick {
    pub fn init(&self, _sys_clock: u32, tick_per_second: u32) -> bool {
        let step = tick_per_second as usize * 1000;
        // SAFETY: step is only written once during initialization
        unsafe {
            *self.step.get() = step;
        }
        boards::set_timeout_after(step);
        true
    }

    pub fn get_tick(&self) -> usize {
        boards::current_ticks()
    }

    pub fn increment_ticks(&self) -> usize {
        boards::current_ticks()
    }

    pub fn get_cycle(&self) -> u64 {
        boards::current_cycles() as u64
    }

    pub fn reset_counter(&self) {
        boards::set_timeout_after(self.get_step());
    }
}
