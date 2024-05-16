use crate::rt_bindings::*;
use core::mem;
use core::ptr;

#[cfg(feature = "RT_USING_SMP")]
static mut CPUS: [rt_cpu; RT_CPUS_NR as usize] = unsafe { mem::zeroed() };

#[cfg(feature = "RT_USING_SMP")]
#[no_mangle]
static mut _cpus_lock: rt_hw_spinlock_t = unsafe { mem::zeroed() };

/// This function will return current CPU object.
///
/// # Returns
///
/// Returns a pointer to the current CPU object.
///
#[cfg(feature = "RT_USING_SMP")]
#[no_mangle]
pub extern "C" fn rt_cpu_self() -> *mut rt_cpu {
    unsafe { ptr::addr_of_mut!(CPUS[rt_hw_cpu_id() as usize]) }
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
pub extern "C" fn rt_cpu_index(index: core::ffi::c_int) -> *mut rt_cpu {
    unsafe { ptr::addr_of_mut!(CPUS[index as usize]) }
}

/// This function will lock all cpus's scheduler and disable local irq.
/// Return current cpu interrupt status.
#[cfg(feature = "RT_USING_SMP")]
#[no_mangle]
pub extern "C" fn rt_cpus_lock() -> rt_base_t {
    unsafe {
        let level = rt_hw_local_irq_disable();
        let current_thread = (*(rt_cpu_self())).current_thread;
        if current_thread != ptr::null_mut() {
            let lock_nest = (*current_thread).cpus_lock_nest;
            (*current_thread).cpus_lock_nest += 1;

            if lock_nest == 0 {
                (*current_thread).scheduler_lock_nest += 1;
                rt_hw_spin_lock(ptr::addr_of_mut!(_cpus_lock));
            }
        }
        level
    }
}

/// This function will restore all cpus's scheduler and restore local irq.
/// level is interrupt status returned by rt_cpus_lock().
#[cfg(feature = "RT_USING_SMP")]
#[no_mangle]
pub extern "C" fn rt_cpus_unlock(level: rt_base_t) {
    unsafe {
        let current_thread = (*(rt_cpu_self())).current_thread;

        if current_thread != ptr::null_mut() {
            assert!((*current_thread).cpus_lock_nest > 0);
            (*current_thread).cpus_lock_nest -= 1;

            if (*current_thread).cpus_lock_nest == 0 {
                (*current_thread).scheduler_lock_nest -= 1;
                rt_hw_spin_unlock(ptr::addr_of_mut!(_cpus_lock));
            }
        }
        rt_hw_local_irq_enable(level);
    }
}

/// This function is invoked by scheduler.
/// It will restore the lock state to whatever the thread's counter expects.
/// If target thread not locked the cpus then unlock the cpus lock.
///
/// thread is a pointer to the target thread.
#[cfg(feature = "RT_USING_SMP")]
#[no_mangle]
pub extern "C" fn rt_cpus_lock_status_restore(thread: *mut rt_thread) {
    unsafe {
        let pcpu = rt_cpu_self();

        #[cfg(all(feature = "ARCH_MM_MMU", feature = "RT_USING_SMART"))]
        lwp_aspace_switch(thread);

        (*pcpu).current_thread = thread;
        if thread != ptr::null_mut() && (*thread).cpus_lock_nest == 0 {
            rt_hw_spin_unlock(ptr::addr_of_mut!(_cpus_lock));
        }
    }
}
