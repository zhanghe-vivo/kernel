use crate::{bluekernel_kconfig::TICK_PER_SECOND, cpu::Cpu, timer};

pub const WAITING_FOREVER: u32 = u32::MAX;

#[doc = "This function will return current tick from operating system startup."]
pub fn get_tick() -> u32 {
    Cpu::get_by_id(0).tick_load()
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
        let tick = TICK_PER_SECOND * (ms as u32 / 1000);
        tick + (TICK_PER_SECOND * (ms as u32 % 1000) + 999) / 1000
    }
}

#[doc = "This function will return the passed millisecond from boot."]
pub fn tick_get_millisecond() -> u32 {
    crate::static_assert!(TICK_PER_SECOND > 0);
    crate::static_assert!(1000 % TICK_PER_SECOND == 0);

    Cpu::get_by_id(0).tick_load() * (1000 / TICK_PER_SECOND)
}
