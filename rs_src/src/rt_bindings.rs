#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

include!(concat!(env!("CARGO_MANIFEST_DIR"), "/bindings.rs"));

#[cfg(not(feature = "RT_USING_SMP"))]
pub const RT_CPUS_NR: u32 = 1;

#[cfg(not(feature = "RT_USING_SMP"))]
#[inline(always)]
pub fn rt_hw_local_irq_disable() -> rt_base_t {
    unsafe { rt_hw_interrupt_disable() }
}

#[cfg(not(feature = "RT_USING_SMP"))]
#[inline(always)]
pub fn rt_hw_local_irq_enable(level: rt_base_t) {
    unsafe { rt_hw_interrupt_enable(level) };
}

#[inline(always)]
pub fn rt_atomic_load(ptr: *mut rt_atomic_t) -> rt_atomic_t {
    unsafe { rt_hw_atomic_load(ptr) }
}

#[inline(always)]
pub fn rt_atomic_store(ptr: *mut rt_atomic_t, val: rt_atomic_t) {
    unsafe {
        rt_hw_atomic_store(ptr, val);
    }
}

#[inline(always)]
pub fn rt_atomic_add(ptr: *mut rt_atomic_t, val: rt_atomic_t) -> rt_atomic_t {
    unsafe { rt_hw_atomic_add(ptr, val) }
}

#[inline(always)]
pub fn rt_atomic_sub(ptr: *mut rt_atomic_t, val: rt_atomic_t) -> rt_atomic_t {
    unsafe { rt_hw_atomic_sub(ptr, val) }
}

#[inline(always)]
pub fn rt_atomic_and(ptr: *mut rt_atomic_t, val: rt_atomic_t) -> rt_atomic_t {
    unsafe { rt_hw_atomic_and(ptr, val) }
}

#[inline(always)]
pub fn rt_atomic_or(ptr: *mut rt_atomic_t, val: rt_atomic_t) -> rt_atomic_t {
    unsafe { rt_hw_atomic_or(ptr, val) }
}

#[inline(always)]
pub fn rt_atomic_xor(ptr: *mut rt_atomic_t, val: rt_atomic_t) -> rt_atomic_t {
    unsafe { rt_hw_atomic_xor(ptr, val) }
}

#[inline(always)]
pub fn rt_atomic_exchange(ptr: *mut rt_atomic_t, val: rt_atomic_t) -> rt_atomic_t {
    unsafe { rt_hw_atomic_exchange(ptr, val) }
}

#[inline(always)]
pub fn rt_atomic_flag_clear(ptr: *mut rt_atomic_t) {
    unsafe {
        rt_hw_atomic_flag_clear(ptr);
    }
}

#[inline(always)]
pub fn rt_atomic_flag_test_and_set(ptr: *mut rt_atomic_t) -> rt_atomic_t {
    unsafe { rt_hw_atomic_flag_test_and_set(ptr) }
}

#[inline(always)]
pub fn rt_atomic_compare_exchange_strong(
    ptr: *mut rt_atomic_t,
    v: *mut rt_atomic_t,
    des: rt_atomic_t,
) -> rt_atomic_t {
    unsafe { rt_hw_atomic_compare_exchange_strong(ptr, v, des) }
}

#[cfg(feature = "RT_USING_SMP")]
#[inline(always)]
pub fn rt_hw_interrupt_disable() -> rt_base_t {
    unsafe { rt_cpus_lock() }
}

#[cfg(feature = "RT_USING_SMP")]
#[inline(always)]
pub fn rt_hw_interrupt_enable(level: rt_base_t) {
    unsafe {
        rt_cpus_unlock(level);
    }
}

#[cfg(not(feature = "RT_USING_SMP"))]
#[inline(always)]
pub unsafe fn rt_hw_spin_lock(lock: *mut rt_spinlock_t) {
    *lock = rt_hw_interrupt_disable();
}

#[cfg(not(feature = "RT_USING_SMP"))]
#[inline(always)]
pub unsafe fn rt_hw_spin_unlock(lock: *mut rt_spinlock_t) {
    rt_hw_interrupt_enable(*lock)
}

#[cfg(not(feature = "RT_USING_HOOK"))]
macro_rules! rt_object_hook_call {
    ($func:ident $(, $argv:expr)?) => {};
}

#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
#[macro_export]
macro_rules! rt_object_hook_call {
    ($func:ident $(, $($argv:expr),* )?) => {
        unsafe {
            if let Some(hook) = $func {
                hook($($($argv),*)?);
            }
        }
    };
}

// macro_rules! rt_object_hook_call {
//     ($func:ident $(, $argv:expr)?) => {
//         unsafe {
//             if let Some(hook) = $func {
//                 hook($(($argv))?);
//             }
//         }
//     };
// }
/// Macro to check current context.
#[cfg(RT_DEBUGING_CONTEXT)]
#[macro_export]
macro_rules! rt_debug_not_in_interrupt {
    () => {{
        let level = rt_hw_interrupt_disable();
        if rt_interrupt_get_nest() != 0 {
            rt_kprintf(
                b"Function[%s] shall not be used in ISR\n",
                core::function!(),
            );
            assert!(0);
        }
        rt_hw_interrupt_enable(level);
    }};
}

///  "In thread context" means:
///    1) the scheduler has been started
///    2) not in interrupt context.
#[cfg(RT_DEBUGING_CONTEXT)]
#[macro_export]
macro_rules! rt_debug_in_thread_context {
    () => {{
        let level: rt_base_t;
        level = rt_hw_interrupt_disable();
        if rt_thread_self().is_null() {
            rt_kprintf(
                b"Function[%s] shall not be used before scheduler start\n",
                core::function!(),
            );
            assert!(0);
        }
        rt_debug_not_in_interrupt!();
        rt_hw_interrupt_enable(level);
    }};
}

/// "scheduler available" means:
/// 1) the scheduler has been started.
/// 2) not in interrupt context.
/// 3) scheduler is not locked.
/// 4) interrupt is not disabled.
#[cfg(RT_DEBUGING_CONTEXT)]
#[macro_export]
macro_rules! rt_debug_scheduler_available {
    ($need_check:expr) => {{
        if $need_check {
            let level: rt_base_t;
            let interrupt_disabled = rt_hw_interrupt_is_disabled();
            let level = rt_hw_interrupt_disable();
            if rt_critical_level() != 0 {
                rt_kprintf(
                    b"Function[%s]: scheduler is not available\n",
                    core::function!(),
                );
                assert!(0);
            }
            if interrupt_disabled {
                rt_kprintf(b"Function[%s]: interrupt is disabled\n", core::function!());
                assert!(0);
            }
            rt_debug_in_thread_context!();
            rt_hw_interrupt_enable(level);
        }
    }};
}

#[cfg(not(RT_DEBUGING_CONTEXT))]
#[macro_export]
macro_rules! rt_debug_not_in_interrupt {
    () => {};
}
#[cfg(not(RT_DEBUGING_CONTEXT))]
#[macro_export]
macro_rules! rt_debug_in_thread_context {
    () => {};
}
#[cfg(not(RT_DEBUGING_CONTEXT))]
#[macro_export]
macro_rules! rt_debug_scheduler_available {
    () => {};
}
