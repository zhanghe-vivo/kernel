use crate::{
    linked_list::ListHead,
    object::{KObjectBase, KernelObject, ObjectClassType},
    print, println, rt_bindings, thread,
};

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

#[no_mangle]
#[allow(unused_unsafe)]
pub extern "C" fn rt_mutex_info() {
    let callback_forword = || {
        println!("mutex      owner  hold priority suspend thread");
        println!("-------- -------- ---- -------- --------------");
    };
    let callback = |node: &ListHead| unsafe {
        let mutex = &*(crate::list_head_entry!(node.as_ptr(), KObjectBase, list)
            as *const rt_bindings::rt_mutex);
        let _ = crate::format_name!(mutex.parent.parent.name.as_ptr(), 8);
        if mutex.owner.is_null() {
            print!(" (NULL)   ");
        } else {
            let _ = crate::format_name!((*mutex.owner).parent.name.as_ptr(), 8);
        }
        print!("{:04}", mutex.hold);
        print!("{:>8}  ", mutex.priority);
        if mutex.parent.suspend_thread.is_empty() {
            println!("0000");
        } else {
            print!("{}:", mutex.parent.suspend_thread.len());
            let head = &mutex.parent.suspend_thread;
            let mut list = head.next;
            loop {
                let thread_node = list;
                if thread_node == head as *const _ as *mut rt_bindings::rt_list_node {
                    break;
                }
                let thread = &*crate::container_of!(thread_node, thread::RtThread, tlist);
                let _ = crate::format_name!(thread.parent.name.as_ptr(), 8);
                list = (*list).next;
            }
            print!("\n");
        }
    };
    let _ = KObjectBase::get_info(
        callback_forword,
        callback,
        ObjectClassType::ObjectClassMutex as u8,
    );
}
