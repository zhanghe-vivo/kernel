mod rt_bindings {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/rt_bindings.rs"));
}

use core::ptr::addr_of;
use core::ptr::addr_of_mut;
use rt_bindings::*;

#[cfg(feature = "RT_USING_SMP")]
static mut RT_TICK: rt_tick_t = (*rt_cpu_index(0)).tick;

#[cfg(not(feature = "RT_USING_SMP"))]
static mut RT_TICK: rt_tick_t = 0;

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
pub extern "C" fn rt_tick_get() -> rt_tick_t {
    unsafe {
        let level = rt_hw_interrupt_disable();
        let tick = core::ptr::read_volatile(addr_of!(RT_TICK));
        rt_hw_interrupt_enable(level);
        tick
    }
}

#[doc = "This function will set current tick."]
#[no_mangle]
pub extern "C" fn rt_tick_set(tick: rt_tick_t) {
    unsafe {
        let level = rt_hw_interrupt_disable();
        core::ptr::write_volatile(addr_of_mut!(RT_TICK), tick);
        rt_hw_interrupt_enable(level);
    }
}

#[doc = "This function will notify kernel there is one tick passed."]
#[no_mangle]
pub extern "C" fn rt_tick_increase() {
    unsafe {
        assert!(rt_interrupt_get_nest() > 0);

        #[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
        {
            if let Some(hook) = unsafe { RT_TICK_HOOK } {
                unsafe {
                    hook();
                }
            }
        }

        let level = rt_hw_interrupt_disable();

        #[cfg(feature = "RT_USING_SMP")]
        {
            (*rt_cpu_self()).tick += 1;
        }

        core::ptr::write_volatile(addr_of_mut!(RT_TICK), RT_TICK + 1);

        /* check time slice */
        let thread = rt_thread_self();
        (*thread).remaining_tick -= 1;

        if (*thread).remaining_tick == 0 {
            /* change to initialized tick */
            (*thread).remaining_tick = (*thread).init_tick;
            (*thread).stat |= RT_THREAD_STAT_YIELD as u8;

            rt_hw_interrupt_enable(level);
            rt_schedule();
        } else {
            rt_hw_interrupt_enable(level);
        }

        rt_timer_check();
    }
}

#[doc = "This function will calculate the tick from millisecond."]
#[no_mangle]
pub extern "C" fn rt_tick_from_millisecond(ms: rt_int32_t) -> rt_tick_t {
    if ms < 0 {
        RT_WAITING_FOREVER as rt_tick_t
    } else {
        let tick = RT_TICK_PER_SECOND * (ms as u32 / 1000);
        tick + (RT_TICK_PER_SECOND * (ms as u32 % 1000) + 999) / 1000
    }
}

#[doc = "This function will return the passed millisecond from boot."]
#[no_mangle]
pub extern "C" fn rt_tick_get_millisecond() -> rt_tick_t {
    static_assert!(RT_TICK_PER_SECOND > 0);
    static_assert!(1000 % RT_TICK_PER_SECOND == 0);

    rt_tick_get() * (1000 / RT_TICK_PER_SECOND) as rt_tick_t
}
