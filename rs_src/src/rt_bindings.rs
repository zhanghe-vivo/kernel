mod gen_bindings {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/bindings.rs"));
}

pub use gen_bindings::*;

#[inline(always)]
pub fn rt_atomic_load(ptr: *mut rt_atomic_t) -> rt_atomic_t {
    unsafe {rt_hw_atomic_load(ptr)}
}

#[inline(always)]
pub fn rt_atomic_store(ptr: *mut rt_atomic_t, val: rt_atomic_t) {
    unsafe {rt_hw_atomic_store(ptr, val);}
}

#[inline(always)]
pub fn rt_atomic_add(ptr: *mut rt_atomic_t, val: rt_atomic_t) -> rt_atomic_t {
    unsafe {rt_hw_atomic_add(ptr, val)}
}

#[inline(always)]
pub fn rt_atomic_sub(ptr: *mut rt_atomic_t, val: rt_atomic_t) -> rt_atomic_t {
    unsafe {rt_hw_atomic_sub(ptr, val)}
}

#[inline(always)]
pub fn rt_atomic_and(ptr: *mut rt_atomic_t, val: rt_atomic_t) -> rt_atomic_t {
    unsafe {rt_hw_atomic_and(ptr, val)}
}

#[inline(always)]
pub fn rt_atomic_or(ptr: *mut rt_atomic_t, val: rt_atomic_t) -> rt_atomic_t {
    unsafe {rt_hw_atomic_or(ptr, val)}
}

#[inline(always)]
pub fn rt_atomic_xor(ptr: *mut rt_atomic_t, val: rt_atomic_t) -> rt_atomic_t {
    unsafe {rt_hw_atomic_xor(ptr, val)}
}

#[inline(always)]
pub fn rt_atomic_exchange(ptr: *mut rt_atomic_t, val: rt_atomic_t) -> rt_atomic_t {
    unsafe {rt_hw_atomic_exchange(ptr, val)}
}

#[inline(always)]
pub fn rt_atomic_flag_clear(ptr: *mut rt_atomic_t) {
    unsafe {rt_hw_atomic_flag_clear(ptr);}
}

#[inline(always)]
pub fn rt_atomic_flag_test_and_set(ptr: *mut rt_atomic_t) -> rt_atomic_t {
    unsafe {rt_hw_atomic_flag_test_and_set(ptr)}
}

#[inline(always)]
pub fn rt_atomic_compare_exchange_strong(ptr: *mut rt_atomic_t,
                                         v: *mut rt_atomic_t,
                                         des: rt_atomic_t)
                                         -> rt_atomic_t {
    unsafe {rt_hw_atomic_compare_exchange_strong(ptr, v, des)}
}

#[cfg(feature = "RT_USING_SMP")]
#[inline(always)]
pub fn rt_hw_interrupt_disable() -> rt_base_t {
    unsafe { rt_cpus_lock() }
}

#[cfg(feature = "RT_USING_SMP")]
#[inline(always)]
pub fn rt_hw_interrupt_enable(level: rt_base_t) {
    unsafe { rt_cpus_unlock(level); }
}