mod rt_bindings {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/rt_bindings.rs"));
}

use core::ptr;
use core::mem;
use rt_bindings::*;

#[cfg(feature = "RT_USING_SMP")]
static mut CPUS: [rt_cpu; RT_CPUS_NR as usize] = unsafe { mem::zeroed() };

#[cfg(feature = "RT_USING_SMP")]
#[no_mangle]
static mut _cpus_lock: rt_hw_spinlock_t =  unsafe { mem::zeroed() };

// Disables preemption for the CPU.
#[cfg(feature = "RT_USING_SMP")]
fn cpu_preempt_disable() {
    unsafe {
        /* disable interrupt */
        let level = rt_hw_local_irq_disable();

        let current_thread = rt_thread_self();
        if current_thread == ptr::null_mut() {
            rt_hw_local_irq_enable(level);
            return;
        }

        /* lock scheduler for local cpu */
        (*current_thread).scheduler_lock_nest += 1;

        /* enable interrupt */
        rt_hw_local_irq_enable(level);
    }
}

/// Enables scheduler for the CPU.
#[cfg(feature = "RT_USING_SMP")]
fn cpu_preempt_enable() {
    unsafe {
        /* disable interrupt */
        let level = rt_hw_local_irq_disable();

        let current_thread = rt_thread_self();
        if current_thread == ptr::null_mut() {
            rt_hw_local_irq_enable(level);
            return;
        }

        /* unlock scheduler for local cpu */
        (*current_thread).scheduler_lock_nest -= 1;

        rt_schedule();
        /* enable interrupt */
        rt_hw_local_irq_enable(level);
    }
}

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
#[no_mangle]
pub extern "C" fn rt_spin_lock_init(lock: *mut rt_spinlock) {
    #[cfg(feature = "RT_USING_SMP")]
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
#[no_mangle]
pub extern "C" fn rt_spin_lock(lock: *mut rt_spinlock) {
    unsafe {
        #[cfg(feature = "RT_USING_SMP")]
        {
            cpu_preempt_disable();
            rt_hw_spin_lock(&mut (*lock).lock);
        }

        #[cfg(not(feature = "RT_USING_SMP"))]
        rt_enter_critical();
    }
}

/// This function will unlock the spinlock, will unlock the thread scheduler.
///
/// # Arguments
///
/// * `lock` - a pointer to the spinlock.
///
#[no_mangle]
pub extern "C" fn rt_spin_unlock(lock: *mut rt_spinlock) {
    unsafe {
        #[cfg(feature = "RT_USING_SMP")]
        {
            rt_hw_spin_unlock(&mut (*lock).lock);
            cpu_preempt_enable();
        }

        #[cfg(not(feature = "RT_USING_SMP"))]
        rt_exit_critical();
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
#[no_mangle]
pub extern "C" fn rt_spin_lock_irqsave(lock: *mut rt_spinlock) -> rt_base_t {
    #[cfg(feature = "RT_USING_SMP")] {
        cpu_preempt_disable();
        unsafe {
            let level = rt_hw_local_irq_disable();
            rt_hw_spin_lock(&mut (*lock).lock);
            level
        }
    }

    #[cfg(not(feature = "RT_USING_SMP"))]
    unsafe { return rt_hw_interrupt_disable(); }
}

/// This function will unlock the spinlock and then restore current CPU interrupt status, will unlock the thread scheduler.
///
/// # Arguments
///
/// * `lock` - a pointer to the spinlock.
/// * `level` - interrupt status returned by rt_spin_lock_irqsave().
///
#[no_mangle]
pub extern "C" fn rt_spin_unlock_irqrestore(lock: *mut rt_spinlock, level: rt_base_t) {
    #[cfg(feature = "RT_USING_SMP")]
    unsafe {
        rt_hw_spin_unlock(&mut (*lock).lock);
        rt_hw_local_irq_enable(level);
        cpu_preempt_enable();
    }

    #[cfg(not(feature = "RT_USING_SMP"))]
    unsafe { rt_hw_interrupt_enable(level);}
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
pub extern "C" fn rt_cpu_index(index: cty::c_int) -> *mut rt_cpu {
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