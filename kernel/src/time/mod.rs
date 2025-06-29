pub(crate) mod systick;
pub(crate) mod timer;

use crate::{arch, scheduler, support::DisableInterruptGuard, thread::Thread};
use bluekernel_kconfig::TICKS_PER_SECOND;
use systick::SYSTICK;

pub const WAITING_FOREVER: usize = usize::MAX;

pub fn systick_init(sys_clock: u32) -> bool {
    SYSTICK.init(sys_clock, TICKS_PER_SECOND as u32)
}

pub fn get_systick() -> usize {
    SYSTICK.get_tick()
}

pub fn get_sysclock_cycle() -> u64 {
    SYSTICK.get_cycle()
}

pub fn reset_systick() {
    SYSTICK.reset_counter();
}

pub extern "C" fn handle_tick_increment() {
    let _ = DisableInterruptGuard::new();
    let mut need_schedule = false;
    // FIXME: aarch64 and riscv64 need to be supported
    if arch::current_cpu_id() == 0 {
        let ticks = SYSTICK.increment_ticks();
        need_schedule = timer::check_hard_timer(ticks);
    }
    need_schedule = scheduler::handle_tick_increment(1) || need_schedule;
    SYSTICK.reset_counter();
    if need_schedule {
        scheduler::yield_me_now_or_later();
    }
}

pub fn tick_from_millisecond(ms: usize) -> usize {
    #[cfg(has_fpu)]
    {
        let ticks = TICKS_PER_SECOND as usize * (ms as usize / 1000);
        ticks + (TICKS_PER_SECOND as usize * (ms as usize % 1000) + 999) / 1000
    }
    // use 1024 as 1000 to aviod use math library
    #[cfg(not(has_fpu))]
    {
        let ticks = (TICKS_PER_SECOND as usize).wrapping_mul(ms as usize >> 10);
        let remainder = ms as usize & 0x3FF;
        ticks.wrapping_add(((TICKS_PER_SECOND as usize).wrapping_mul(remainder) + 1023) >> 10)
    }
}

pub fn tick_to_millisecond(ticks: usize) -> usize {
    ticks * (1000 / TICKS_PER_SECOND)
}

pub fn tick_get_millisecond() -> usize {
    crate::static_assert!(TICKS_PER_SECOND > 0);
    crate::static_assert!(1000 % TICKS_PER_SECOND == 0);

    get_systick() * (1000 / TICKS_PER_SECOND)
}
