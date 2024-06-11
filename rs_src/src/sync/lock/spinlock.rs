#![allow(dead_code)]
use crate::rt_bindings::*;

impl rt_spinlock {
    pub const fn new() -> Self {
        Self {
            #[cfg(feature = "RT_USING_SMP")]
            lock: rt_hw_spinlock_t { slock: 0 },

            #[cfg(not(feature = "RT_USING_SMP"))]
            lock: 0,
        }
    }

    pub fn acquire(&self) -> RtSpinGuard<'_> {
        self.lock();
        RtSpinGuard(self)
    }

    pub fn lock(&self) {
        #[cfg(feature = "RT_USING_SMP")]
        unsafe {
            Self::cpu_preempt_disable();
            rt_hw_spin_lock((&self.lock) as *const _ as *mut _);
        }

        #[cfg(not(feature = "RT_USING_SMP"))]
        unsafe {
            rt_enter_critical()
        };
    }

    pub fn unlock(&self) {
        #[cfg(feature = "RT_USING_SMP")]
        unsafe {
            rt_hw_spin_unlock((&self.lock) as *const _ as *mut _);
            Self::cpu_preempt_enable();
        }

        #[cfg(not(feature = "RT_USING_SMP"))]
        unsafe {
            rt_exit_critical()
        };
    }

    pub fn lock_irqsave(&self) -> rt_base_t {
        #[cfg(feature = "RT_USING_SMP")]
        unsafe {
            Self::cpu_preempt_disable();
            let level = rt_hw_local_irq_disable();
            rt_hw_spin_lock((&self.lock) as *const _ as *mut _);
            level
        }

        #[cfg(not(feature = "RT_USING_SMP"))]
        unsafe {
            rt_hw_interrupt_disable()
        }
    }

    pub fn unlock_irqrestore(&self, level: rt_base_t) {
        #[cfg(feature = "RT_USING_SMP")]
        unsafe {
            rt_hw_spin_unlock((&self.lock) as *const _ as *mut _);
            rt_hw_local_irq_enable(level);
            Self::cpu_preempt_enable();
        }

        #[cfg(not(feature = "RT_USING_SMP"))]
        unsafe {
            rt_hw_interrupt_enable(level)
        };
    }

    // Disables preemption for the CPU.
    #[cfg(feature = "RT_USING_SMP")]
    unsafe fn cpu_preempt_disable() {
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

    /// Enables scheduler for the CPU.
    #[cfg(feature = "RT_USING_SMP")]
    unsafe fn cpu_preempt_enable() {
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

pub struct RtSpinGuard<'a>(&'a rt_spinlock);

impl Drop for RtSpinGuard<'_> {
    #[inline]
    fn drop(&mut self) {
        self.0.unlock();
    }
}

/// Initialize a static spinlock object.
///
/// # Arguments
///
/// * `lock` - a pointer to the spinlock to initialize.
///
#[no_mangle]
pub unsafe extern "C" fn rt_spin_lock_init(lock: *mut rt_spinlock) {
    #[cfg(feature = "RT_USING_SMP")]
    unsafe {
        rt_hw_spin_lock_init(&mut (*lock).lock)
    };
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
pub unsafe extern "C" fn rt_spin_lock(lock: *mut rt_spinlock) {
    (*lock).lock();
}

/// This function will unlock the spinlock, will unlock the thread scheduler.
///
/// # Arguments
///
/// * `lock` - a pointer to the spinlock.
///
#[no_mangle]
pub unsafe extern "C" fn rt_spin_unlock(lock: *mut rt_spinlock) {
    (*lock).unlock();
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
pub unsafe extern "C" fn rt_spin_lock_irqsave(lock: *mut rt_spinlock) -> rt_base_t {
    (*lock).lock_irqsave()
}

/// This function will unlock the spinlock and then restore current CPU interrupt status, will unlock the thread scheduler.
///
/// # Arguments
///
/// * `lock` - a pointer to the spinlock.
/// * `level` - interrupt status returned by rt_spin_lock_irqsave().
///
#[no_mangle]
pub unsafe extern "C" fn rt_spin_unlock_irqrestore(lock: *mut rt_spinlock, level: rt_base_t) {
    (*lock).unlock_irqrestore(level);
}

#[macro_export]
macro_rules! new_spinlock {
    ($inner:expr $(, $name:literal)? $(,)?) => {
        $crate::sync::SpinLock::new(
            $inner, $crate::optional_name!($($name)?))
    };
}
pub use new_spinlock;

pub type SpinLock<T> = super::Lock<T, SpinLockBackend>;

pub struct SpinLockBackend;

// SAFETY: The underlying kernel `spinlock_t` object ensures mutual exclusion. `relock` uses the
// default implementation that always calls the same locking method.
unsafe impl super::Backend for SpinLockBackend {
    type State = rt_spinlock;
    type GuardState = ();

    unsafe fn init(ptr: *mut Self::State, _name: *const core::ffi::c_char) {
        // SAFETY: The safety requirements ensure that `ptr` is valid for writes, and `name` and
        // `key` are valid for read indefinitely.
        unsafe { rt_spin_lock_init(ptr) }
    }

    unsafe fn lock(ptr: *mut Self::State) -> Self::GuardState {
        // SAFETY: The safety requirements of this function ensure that `ptr` points to valid
        // memory, and that it has been initialised before.
        unsafe { rt_spin_lock(ptr) }
    }

    unsafe fn unlock(ptr: *mut Self::State, _guard_state: &Self::GuardState) {
        // SAFETY: The safety requirements of this function ensure that `ptr` is valid and that the
        // caller is the owner of the spinlock.
        unsafe { rt_spin_unlock(ptr) }
    }
}
