use crate::{cpu::Cpu, timer};
use rt_bindings;

#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
static mut RT_TICK_HOOK: Option<unsafe extern "C" fn()> = None;

#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
#[no_mangle]
pub extern "C" fn rt_tick_sethook(hook: unsafe extern "C" fn()) {
    unsafe {
        RT_TICK_HOOK = Some(hook);
    }
}

#[doc = "This function will return current tick from operating system startup."]
#[no_mangle]
pub extern "C" fn rt_tick_get() -> rt_bindings::rt_tick_t {
    Cpu::get_by_id(0).tick_load()
}

#[doc = "This function will set current tick."]
#[no_mangle]
pub extern "C" fn rt_tick_set(tick: rt_bindings::rt_tick_t) {
    Cpu::get_by_id(0).tick_store(tick);
}

#[doc = "This function will notify kernel there is one tick passed."]
#[no_mangle]
pub extern "C" fn rt_tick_increase() {
    unsafe {
        assert!(Cpu::interrupt_nest_load() > 0);

        rt_bindings::rt_object_hook_call!(RT_TICK_HOOK);

        Cpu::get_current().tick_inc();
        /* check time slice */
        let scheduler = Cpu::get_current_scheduler();
        scheduler.handle_tick_increase();

        timer::rt_timer_check();
    }
}

#[doc = "This function will calculate the tick from millisecond."]
#[no_mangle]
pub extern "C" fn rt_tick_from_millisecond(ms: rt_bindings::rt_int32_t) -> rt_bindings::rt_tick_t {
    if ms < 0 {
        rt_bindings::RT_WAITING_FOREVER as rt_bindings::rt_tick_t
    } else {
        let tick = rt_bindings::RT_TICK_PER_SECOND * (ms as u32 / 1000);
        tick + (rt_bindings::RT_TICK_PER_SECOND * (ms as u32 % 1000) + 999) / 1000
    }
}

#[doc = "This function will return the passed millisecond from boot."]
#[no_mangle]
pub extern "C" fn rt_tick_get_millisecond() -> rt_bindings::rt_tick_t {
    crate::static_assert!(rt_bindings::RT_TICK_PER_SECOND > 0);
    crate::static_assert!(1000 % rt_bindings::RT_TICK_PER_SECOND == 0);

    Cpu::get_by_id(0).tick_load()
        * (1000 / rt_bindings::RT_TICK_PER_SECOND) as rt_bindings::rt_tick_t
}
