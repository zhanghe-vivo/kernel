#[cfg(feature = "RT_USING_CONDVAR")]
pub mod condvar;
#[cfg(feature = "RT_USING_EVENT")]
pub mod event;
pub mod ipc_common;
pub mod lock;
#[cfg(feature = "RT_USING_MAILBOX")]
pub mod mailbox;
#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
pub mod message_queue;
#[cfg(feature = "RT_USING_SEMAPHORE")]
pub mod semaphore;

pub use lock::{
    heaplock::{new_heaplock, HeapLock},
    spinlock::{new_spinlock, RawSpin, SpinLock, SpinMutex},
};

/// Returns the given string, if one is provided, otherwise generates one based on the source code
/// location.
#[doc(hidden)]
#[macro_export]
macro_rules! optional_name {
    () => {
        $crate::c_str!(::core::concat!(::core::file!(), ":", ::core::line!()))
    };
    ($name:literal) => {
        $crate::c_str!($name)
    };
}
