#![allow(dead_code)]
use crate::rt_bindings::*;
use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    ptr,
};

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

/// Initialize a static spinlock object.
///
/// # Arguments
///
/// * `lock` - a pointer to the spinlock to initialize.
///
#[no_mangle]
pub extern "C" fn rt_spin_lock_init(lock: *mut rt_spinlock) {
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
pub extern "C" fn rt_spin_lock(lock: *mut rt_spinlock) {
    #[cfg(feature = "RT_USING_SMP")]
    unsafe {
        cpu_preempt_disable();
        rt_hw_spin_lock(&mut (*lock).lock);
    }

    #[cfg(not(feature = "RT_USING_SMP"))]
    unsafe {
        rt_enter_critical()
    };
}

/// This function will unlock the spinlock, will unlock the thread scheduler.
///
/// # Arguments
///
/// * `lock` - a pointer to the spinlock.
///
#[no_mangle]
pub extern "C" fn rt_spin_unlock(lock: *mut rt_spinlock) {
    #[cfg(feature = "RT_USING_SMP")]
    unsafe {
        rt_hw_spin_unlock(&mut (*lock).lock);
        cpu_preempt_enable();
    }

    #[cfg(not(feature = "RT_USING_SMP"))]
    unsafe {
        rt_exit_critical()
    };
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
    #[cfg(feature = "RT_USING_SMP")]
    unsafe {
        cpu_preempt_disable();
        let level = rt_hw_local_irq_disable();
        rt_hw_spin_lock(&mut (*lock).lock);
        level
    }

    #[cfg(not(feature = "RT_USING_SMP"))]
    unsafe {
        rt_hw_interrupt_disable()
    }
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
    unsafe {
        rt_hw_interrupt_enable(level)
    };
}

impl rt_spinlock {
    pub const fn new() -> Self {
        Self {
            #[cfg(feature = "RT_USING_SMP")]
            lock: rt_hw_spinlock_t { slock: 0 },

            #[cfg(not(feature = "RT_USING_SMP"))]
            lock: 0,
        }
    }
}

pub struct SpinLock<T: ?Sized> {
    lock: rt_spinlock,
    data: UnsafeCell<T>,
}

/// A guard that provides mutable data access.
///
/// When the guard falls out of scope it will release the lock.
pub struct SpinLockGuard<'a, T: ?Sized + 'a> {
    lock: &'a rt_spinlock,
    data: *mut T,
}

// Same unsafe impls as `std::sync::Mutex`
unsafe impl<T: ?Sized + Send> Sync for SpinLock<T> {}
unsafe impl<T: ?Sized + Send> Send for SpinLock<T> {}

unsafe impl<T: ?Sized + Sync> Sync for SpinLockGuard<'_, T> {}
unsafe impl<T: ?Sized + Send> Send for SpinLockGuard<'_, T> {}

impl<T> SpinLock<T> {
    #[inline(always)]
    pub const fn new(t: T) -> Self {
        Self {
            lock: rt_spinlock::new(),
            data: UnsafeCell::new(t),
        }
    }

    #[inline(always)]
    pub fn into_inner(self) -> T {
        // We know statically that there are no outstanding references to
        // `self` so there's no need to lock.
        let SpinLock { data, .. } = self;
        data.into_inner()
    }

    #[inline(always)]
    pub fn as_mut_ptr(&self) -> *mut T {
        self.data.get()
    }

    #[inline(always)]
    pub fn lock(&self) -> SpinLockGuard<T> {
        unsafe {
            rt_spin_lock(&self.lock as *const _ as *mut _);
        }
        SpinLockGuard {
            lock: &self.lock,
            data: unsafe { &mut *self.data.get() },
        }
    }

    fn drop(&mut self) {
        unsafe {
            self.as_mut_ptr().drop_in_place();
        }
    }
}

impl<T: ?Sized + Default> Default for SpinLock<T> {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl<T> From<T> for SpinLock<T> {
    fn from(data: T) -> Self {
        Self::new(data)
    }
}

impl<'a, T: ?Sized> Deref for SpinLockGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        // We know statically that only we are referencing data
        unsafe { &*self.data }
    }
}

impl<'a, T: ?Sized> DerefMut for SpinLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        // We know statically that only we are referencing data
        unsafe { &mut *self.data }
    }
}

impl<'a, T: ?Sized> Drop for SpinLockGuard<'a, T> {
    /// The dropping of the MutexGuard will release the lock it was created from.
    fn drop(&mut self) {
        unsafe {
            rt_spin_unlock(self.lock as *const _ as *mut _);
        }
    }
}
