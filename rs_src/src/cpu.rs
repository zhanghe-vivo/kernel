mod rt_bindings {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/rt_bindings.rs"));
}

use core::ptr;
use rt_bindings::*;

#[cfg(feature = "RT_USING_DEBUG")]
static mut CPUS_CRITICAL_LEVEL: rt_base_t = 0;

#[cfg(feature = "RT_USING_SMP")]
static mut CPUS: [rt_cpu; RT_CPUS_NR] = [rt_cpu {}; RT_CPUS_NR];

#[cfg(feature = "RT_USING_SMP")]
static mut CPUS_LOCK: rt_hw_spinlock_t =  {};

#[cfg(all(feature = "RT_USING_SMP", feature = "RT_DEBUGING_SPINLOCK"))]
static CPUS_LOCK_OWNER: *mut rt_thread = 0;

#[cfg(all(feature = "RT_USING_SMP", feature = "RT_DEBUGING_SPINLOCK"))]
static CPUS_LOCK_PC: usize = 0;

/// Initialize a static spinlock object.
///
/// # Arguments
///
/// * `lock` - a pointer to the spinlock to initialize.
///
/// # Example
///
/// ```rust
/// // Initialize a spinlock
/// let mut spinlock = Spinlock::new();
/// ```
#[cfg(feature = "RT_USING_SMP")]
#[no_mangle]
pub extern "C" fn rt_spin_lock_init(lock: *mut rt_spinlock) {
    unsafe { rt_hw_spin_lock_init(&mut (*lock).lock) };
}

/// This function will lock the spinlock, will lock the thread scheduler.
///
/// If the spinlock is locked, the current CPU will keep polling the spinlock state
/// until the spinlock is unlocked.
///
/// # Arguments
///
/// * `lock` - a pointer to the spinlock.
///
#[cfg(feature = "RT_USING_SMP")]
#[no_mangle]
pub extern "C" fn rt_spin_lock(lock: *mut rt_spinlock) {
    unsafe {
        rt_enter_critical();
        rt_hw_spin_lock(&mut (*lock).lock);
        #[cfg(feature = "RT_DEBUGING_SPINLOCK")]
        rt_spin_lock_debug(lock);
    }
}

/// This function will unlock the spinlock, will unlock the thread scheduler.
///
/// # Arguments
///
/// * `lock` - a pointer to the spinlock.
///
#[cfg(feature = "RT_USING_SMP")]
#[no_mangle]
pub extern "C" fn rt_spin_unlock(lock: *mut rt_spinlock) {
    unsafe {
        let mut critical_level: rt_base_t = 0;
        #[cfg(feature = "RT_DEBUGING_SPINLOCK")]
        rt_spin_unlock_debug(lock, &mut critical_level);
    
        rt_hw_spin_unlock(&mut (*lock).lock);
        rt_exit_critical_safe(critical_level);
    }
}

/// This function will disable the local interrupt and then lock the spinlock, will lock the thread scheduler.
///
/// If the spinlock is locked, the current CPU will keep polling the spinlock state
/// until the spinlock is unlocked.
///
/// # Arguments
///
/// * `lock` - a pointer to the spinlock.
///
/// # Returns
///
/// Returns current CPU interrupt status.
///
#[cfg(feature = "RT_USING_SMP")]
#[no_mangle]
pub extern "C" fn rt_spin_lock_irqsave(lock: *mut rt_spinlock) -> rt_base_t {
    let level: rt_base_t;
    unsafe {
        level = rt_hw_local_irq_disable();
        rt_enter_critical();
        rt_hw_spin_lock(&mut (*lock).lock);

        #[cfg(feature = "RT_DEBUGING_SPINLOCK")]
        RT_SPIN_LOCK_DEBUG(lock);
    }
    level
}

/// This function will unlock the spinlock and then restore current CPU interrupt status, will unlock the thread scheduler.
///
/// # Arguments
///
/// * `lock` - a pointer to the spinlock.
/// * `level` - interrupt status returned by rt_spin_lock_irqsave().
///
#[cfg(feature = "RT_USING_SMP")]
#[no_mangle]
pub extern "C" fn rt_spin_unlock_irqrestore(lock: *mut rt_spinlock, level: rt_base_t) {
    unsafe {
        let mut critical_level: rt_base_t = 0;

        #[cfg(feature = "RT_DEBUGING_SPINLOCK")]
        RT_SPIN_UNLOCK_DEBUG(lock, &mut critical_level);

        rt_hw_spin_unlock(&mut (*lock).lock);
        rt_hw_local_irq_enable(level);
        rt_exit_critical_safe(critical_level);
    }
}

/// This function will return current CPU object.
///
/// # Returns
///
/// Returns a pointer to the current CPU object.
///
#[cfg(feature = "RT_USING_SMP")]
#[no_mangle]
pub extern "C" fn rt_cpu_self() -> *mut rt_cpu {
    &CPUS[rt_hw_cpu_id() as usize]
}

/// This function will return the CPU object corresponding to the index.
///
/// # Arguments
///
/// * `index` - the index of the target CPU object.
///
/// # Returns
///
/// Returns a pointer to the CPU object corresponding to the index.
///
#[cfg(feature = "RT_USING_SMP")]
#[no_mangle]
pub extern "C" fn rt_cpu_index(index: c_int) -> *mut rt_cpu {
    &CPUS[index as usize]
}

/// This function will lock all cpus's scheduler and disable local irq.
/// Return current cpu interrupt status.
#[cfg(feature = "RT_USING_SMP")]
#[no_mangle]
pub extern "C" fn rt_cpus_lock() -> rt_base_t {
    let level = rt_hw_local_irq_disable();
    let current_thread = *(rt_cpu_self()).current_thread;
    if current_thread != ptr::null() {
        // TODO: 是否要将rt的atomic实现封装为rust的atomic
        let lock_nest = rt_atomic_load(current_thread.cpus_lock_nest);
        rt_atomic_add(current_thread.cpus_lock_nest, 1);

        if lock_nest == 0 {
            rt_enter_critical();
            rt_hw_spin_lock(&_cpus_lock);
            #[cfg(feature = "RT_USING_DEBUG")]
            CPUS_CRITICAL_LEVEL = rt_critical_level();
            #[cfg(feature = "RT_DEBUGING_SPINLOCK")]
            CPUS_LOCK_OWNER = current_thread;
            CPUS_LOCK_PC = caller_address!();
        }
    }

    level
}

/// This function will restore all cpus's scheduler and restore local irq.
/// level is interrupt status returned by rt_cpus_lock().
#[cfg(feature = "RT_USING_SMP")]
#[no_mangle]
pub extern "C" fn rt_cpus_unlock(level: rt_base_t) {
    let current_thread = *(rt_cpu_self()).current_thread;

    if current_thread != ptr::null() {
        assert!(rt_atomic_load(current_thread.cpus_lock_nest) > 0);
        rt_atomic_sub(current_thread.cpus_lock_nest, 1);

        if current_thread.cpus_lock_nest == 0 {
            #[cfg(feature = "RT_DEBUGING_SPINLOCK")]
            _cpus_lock_owner = __OWNER_MAGIC;
            _cpus_lock_pc = RT_NULL;
            #[cfg(feature = "RT_USING_DEBUG")]
            let critical_level = CPUS_CRITICAL_LEVEL;
            CPUS_CRITICAL_LEVEL = 0;
            rt_hw_spin_unlock(&_cpus_lock);
            rt_exit_critical_safe(critical_level);
        }
    }
    rt_hw_local_irq_enable(level);
}

