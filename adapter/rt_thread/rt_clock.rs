use crate::bluekernel::{clock, cpu::Cpu};

#[doc = "This function will return current tick from operating system startup."]
#[no_mangle]
pub extern "C" fn rt_tick_get() -> u32 {
    Cpu::get_by_id(0).tick_load()
}

#[doc = "This function will set current tick."]
#[no_mangle]
pub extern "C" fn rt_tick_set(tick: u32) {
    Cpu::get_by_id(0).tick_store(tick);
}

#[doc = "This function will notify kernel there is one tick passed."]
#[no_mangle]
pub extern "C" fn rt_tick_increase() {
    clock::handle_tick_increase();
}

#[doc = "This function will calculate the tick from millisecond."]
#[no_mangle]
pub extern "C" fn rt_tick_from_millisecond(ms: i32) -> u32 {
    clock::tick_from_millisecond(ms)
}

#[doc = "This function will return the passed millisecond from boot."]
#[no_mangle]
pub extern "C" fn rt_tick_get_millisecond() -> u32 {
    clock::tick_get_millisecond()
}
