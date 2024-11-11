use crate::{rt_bindings, sync::lock::mutex::*, thread};

#[cfg(feature = "RT_USING_HEAP_ISR")]
use crate::sync::lock::spinlock::SpinLockBackend;
#[cfg(feature = "RT_USING_HEAP_ISR")]
pub type HeapLock<T> = super::Lock<T, SpinLockBackend>;
#[cfg(feature = "RT_USING_HEAP_ISR")]
pub use crate::new_spinlock as new_heaplock;

#[cfg(all(not(feature = "RT_USING_HEAP_ISR"), feature = "RT_USING_MUTEX"))]
pub type HeapLock<T> = super::Lock<T, HeapLockBackend>;
#[cfg(all(not(feature = "RT_USING_HEAP_ISR"), feature = "RT_USING_MUTEX"))]
#[macro_export]
macro_rules! new_heaplock {
    ($inner:expr $(, $name:literal)? $(,)?) => {
        $crate::sync::HeapLock::new(
            $inner, $crate::optional_name!($($name)?))
    };
}
#[cfg(all(not(feature = "RT_USING_HEAP_ISR"), feature = "RT_USING_MUTEX"))]
pub use new_heaplock;

/// A kernel `struct mutex` lock backend.
pub struct HeapLockBackend;

// SAFETY: The underlying kernel `struct mutex` object ensures mutual exclusion.
unsafe impl super::Backend for HeapLockBackend {
    type State = RtMutex;
    type GuardState = ();

    unsafe fn init(ptr: *mut Self::State, name: *const core::ffi::c_char) {
        // SAFETY: The safety requirements ensure that `ptr` is valid for writes, and `name` and
        // `key` are valid for read indefinitely.
        unsafe { rt_mutex_init(ptr, name, rt_bindings::RT_IPC_FLAG_PRIO as u8) };
    }

    unsafe fn lock(ptr: *mut Self::State) -> Self::GuardState {
        // SAFETY: The safety requirements of this function ensure that `ptr` points to valid
        // memory, and that it has been initialised before.
        unsafe {
            if !thread::rt_thread_self().is_null() {
                rt_mutex_take(ptr, rt_bindings::RT_WAITING_FOREVER)
            } else {
                rt_bindings::RT_EOK as i32
            }
        };
    }

    unsafe fn unlock(ptr: *mut Self::State, _guard_state: &Self::GuardState) {
        // SAFETY: The safety requirements of this function ensure that `ptr` is valid and that the
        // caller is the owner of the mutex.
        unsafe {
            if !thread::rt_thread_self().is_null() {
                rt_mutex_release(ptr);
            }
        }
    }
}
