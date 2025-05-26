#[cfg(feature = "condvar")]
pub mod condvar;
#[cfg(feature = "event")]
pub mod event;
pub mod futex;
pub mod ipc_common;
pub mod lock;
#[cfg(feature = "mailbox")]
pub mod mailbox;
#[cfg(feature = "messagequeue")]
pub mod message_queue;
#[cfg(feature = "semaphore")]
pub mod semaphore;
pub mod wait_list;

pub use lock::spinlock::{RawSpin, SpinLock};

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
