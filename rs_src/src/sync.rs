pub mod lock;
pub mod ipc_common;
pub mod semaphore;
pub mod event;
pub mod mailbox;
pub mod message_queue;

pub use lock::heaplock::{new_heaplock, HeapLock};
pub use lock::mutex::{new_mutex, Mutex};
pub use lock::spinlock::{new_spinlock, SpinLock};

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


