use crate::rt_bindings;

// #[repr(C)]
// #[cfg_attr(target_pointer_width = "16", repr(align(8)))]
// #[cfg_attr(target_pointer_width = "32", repr(align(16)))]
// #[cfg_attr(target_pointer_width = "64", repr(align(32)))]
// #[derive(Debug)]
// #[pin_data]
// pub struct RawMutex {
//     lock: RawSpin,

//     // kernel base object, can't delete
//     #[pin]
//     parent: rt_bindings::rt_object,

//     owner: *mut RtThread,
//     /// list of tasks wait for this mutex.
//     #[pin]
//     wait_list: ListHead,
//     /// list of mutex hold by thread. use to find right priority.
//     #[pin]
//     take_list: ListHead,
//     /// the priority ceiling of mutexe
//     ceiling_priority: ffi::c_uchar,
//     /// the maximal priority for pending thread
//     priority: ffi::c_uchar,
//     /// numbers of thread hold the mutex
//     count: ffi::c_uchar,
// }

#[macro_export]
macro_rules! new_mutex {
    ($inner:expr $(, $name:literal)? $(,)?) => {
        $crate::sync::Mutex::new(
            $inner, $crate::optional_name!($($name)?))
    };
}
pub use new_mutex;

pub type Mutex<T> = super::Lock<T, MutexBackend>;

/// A kernel `struct mutex` lock backend.
pub struct MutexBackend;

// SAFETY: The underlying kernel `struct mutex` object ensures mutual exclusion.
unsafe impl super::Backend for MutexBackend {
    type State = rt_bindings::rt_mutex;
    type GuardState = ();

    unsafe fn init(ptr: *mut Self::State, name: *const core::ffi::c_char) {
        // SAFETY: The safety requirements ensure that `ptr` is valid for writes, and `name` and
        // `key` are valid for read indefinitely.
        unsafe { rt_bindings::rt_mutex_init(ptr, name, rt_bindings::RT_IPC_FLAG_PRIO as u8) };
    }

    unsafe fn lock(ptr: *mut Self::State) -> Self::GuardState {
        // SAFETY: The safety requirements of this function ensure that `ptr` points to valid
        // memory, and that it has been initialised before.
        unsafe { rt_bindings::rt_mutex_take(ptr, rt_bindings::RT_WAITING_FOREVER) };
    }

    unsafe fn unlock(ptr: *mut Self::State, _guard_state: &Self::GuardState) {
        // SAFETY: The safety requirements of this function ensure that `ptr` is valid and that the
        // caller is the owner of the mutex.
        unsafe { rt_bindings::rt_mutex_release(ptr) };
    }
}
