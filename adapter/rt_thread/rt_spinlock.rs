use crate::kernel::sync::lock::spinlock::RawSpin;

/// Initialize a static spinlock object.
///
/// # Arguments
///
/// * `lock` - a pointer to the spinlock to initialize.
///
#[no_mangle]
pub unsafe extern "C" fn rt_spin_lock_init(_spin: *mut RawSpin) {
    #[cfg(smp)]
    unsafe {
        //TODO:add rawspinlock init
        // rt_bindings::rt_hw_spin_lock_init(&mut (*_spin).lock)
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
pub unsafe extern "C" fn rt_spin_lock(spin: *mut RawSpin) {
    // let raw = spin as *mut RawSpin;
    (*spin).lock();
}

/// This function will unlock the spinlock, will unlock the thread scheduler.
///
/// # Arguments
///
/// * `lock` - a pointer to the spinlock.
///
#[no_mangle]
pub unsafe extern "C" fn rt_spin_unlock(spin: *mut RawSpin) {
    // let raw = spin as *mut RawSpin;
    (*spin).unlock();
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
pub unsafe extern "C" fn rt_spin_lock_irqsave(spin: *mut RawSpin) -> usize {
    // let raw = spin as *mut RawSpin;
    (*spin).lock_irqsave()
}

/// This function will unlock the spinlock and then restore current CPU interrupt status, will unlock the thread scheduler.
///
/// # Arguments
///
/// * `lock` - a pointer to the spinlock.
/// * `level` - interrupt status returned by rt_spin_lock_irqsave().
///
#[no_mangle]
pub unsafe extern "C" fn rt_spin_unlock_irqrestore(spin: *mut RawSpin, level: usize) {
    // let raw = spin as *mut RawSpin;
    (*spin).unlock_irqrestore(level);
}
