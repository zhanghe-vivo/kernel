use crate::{
    arch,
    bluekernel_kconfig::TICK_PER_SECOND,
    boards::{boot_time, config},
    clock, timer,
};
use core::sync::atomic::Ordering;

pub const WAITING_FOREVER: u32 = u32::MAX;

#[doc = "This function will return current tick from operating system startup."]
pub fn get_tick() -> u32 {
    Cpu::get_by_id(0).tick_load()
}

#[cfg(cortex_m)]
/// Return the cycle counts since the boot.
pub(crate) fn get_clock_cycle() -> u64 {
    let systick_val = Arch::get_systick_value();
    let tick = clock::get_tick() as u64;
    let systick_reload = Arch::get_systick_reload();
    (systick_reload + 1) * tick + systick_reload.saturating_sub(systick_val)
}

#[cfg(not(cortex_m))]
/// Return the cycle counts since the boot.
pub(crate) fn get_clock_cycle() -> u64 {
    let current_cycle_count: u64 = Arch::get_cycle_count();
    let boot_cycle_count: u64 = BOOT_CYCLE_COUNT.load(Ordering::Relaxed);
    current_cycle_count.saturating_sub(boot_cycle_count)
}

/// Convert the cycle counts to the time in units of 10ms.
pub(crate) fn cycle_to_10ms(cycle: u64) -> u64 {
    let cycle_per_10ms: u64 = sys_config::get_system_core_clock() / 100;
    (cycle + cycle_per_10ms / 2) / cycle_per_10ms
}

#[doc = "This function will set current tick."]
pub fn set_tick(tick: u32) {
    Cpu::get_by_id(0).tick_store(tick);
}

#[doc = "This function will notify kernel there is one tick passed."]
pub fn handle_tick_increase() {
    assert!(Cpu::interrupt_nest_load() > 0);
    Cpu::get_current().tick_inc();
    /* check time slice */
    let scheduler = Cpu::get_current_scheduler();
    scheduler.handle_tick_increase();
    timer::timer_check();
}

#[doc = "This function will calculate the tick from millisecond."]
pub fn tick_from_millisecond(ms: i32) -> u32 {
    if ms < 0 {
        WAITING_FOREVER
    } else {
        // use fp
        #[cfg(has_fpu)]
        {
            let tick = TICK_PER_SECOND as u32 * (ms as u32 / 1000);
            tick + (TICK_PER_SECOND as u32 * (ms as u32 % 1000) + 999) / 1000
        }
        // use 1024 as 1000 to aviod use math library
        #[cfg(not(has_fpu))]
        {
            let tick = (TICK_PER_SECOND as u32).wrapping_mul(ms as u32 >> 10);
            let remainder = ms as u32 & 0x3FF;
            tick.wrapping_add(((TICK_PER_SECOND as u32).wrapping_mul(remainder) + 1023) >> 10)
        }
    }
}

#[doc = "This function will return the passed millisecond from boot."]
pub fn tick_get_millisecond() -> u32 {
    crate::static_assert!(TICK_PER_SECOND > 0);
    crate::static_assert!(1000 % TICK_PER_SECOND == 0);

    Cpu::get_by_id(0).tick_load() * (1000 / TICK_PER_SECOND as u32)
}
