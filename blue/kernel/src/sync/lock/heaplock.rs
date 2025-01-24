use crate::{cpu::Cpu, sync::lock::mutex::*};

#[cfg(feature = "heap_isr")]
use crate::sync::lock::spinlock::SpinLockBackend;
#[cfg(feature = "heap_isr")]
pub type HeapLock<T> = super::Lock<T, SpinLockBackend>;
#[cfg(feature = "heap_isr")]
pub use crate::new_spinlock as new_heaplock;

#[cfg(not(feature = "heap_isr"))]
pub type HeapLock<T> = super::Lock<T, HeapLockBackend>;
#[cfg(not(feature = "heap_isr"))]
#[macro_export]
macro_rules! new_heaplock {
    ($inner:expr $(, $name:literal)? $(,)?) => {
        $crate::sync::HeapLock::new(
            $inner, $crate::optional_name!($($name)?))
    };
}
#[cfg(not(feature = "heap_isr"))]
pub use new_heaplock;

/// A kernel `struct mutex` lock backend.
pub struct HeapLockBackend;

// SAFETY: The underlying kernel `struct mutex` object ensures mutual exclusion.
unsafe impl super::Backend for HeapLockBackend {
    type State = Mutex;
    type GuardState = ();

    unsafe fn init(ptr: *mut Self::State, name: *const core::ffi::c_char) {
        // SAFETY: The safety requirements ensure that `ptr` is valid for writes, and `name` and
        // `key` are valid for read indefinitely.
        unsafe { (*ptr).init(name) };
    }

    unsafe fn lock(ptr: *mut Self::State) -> Self::GuardState {
        // SAFETY: The safety requirements of this function ensure that `ptr` points to valid
        // memory, and that it has been initialised before.
        unsafe {
            let _ = if Cpu::get_current_thread().is_some() {
                (*ptr).lock()
            } else {
                Ok(())
            };
        };
    }

    unsafe fn unlock(ptr: *mut Self::State, _guard_state: &Self::GuardState) {
        // SAFETY: The safety requirements of this function ensure that `ptr` is valid and that the
        // caller is the owner of the mutex.
        unsafe {
            if Cpu::get_current_thread().is_some() {
                let _ = (*ptr).unlock();
            }
        }
    }
}
