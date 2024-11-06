use crate::cpu::{self, Cpu, Cpus};
use crate::linked_list::*;
use crate::object::{
    rt_object_get_type, rt_object_put_hook, rt_object_take_hook, rt_object_trytake_hook,
    BaseObject, ObjectClassType,
};
use crate::rt_bindings::{self, *};
use crate::sync::ipc_common::*;
use crate::thread::{rt_thread_control, RtThread};
use crate::{
    current_thread_ptr, list_head_for_each, rt_debug_in_thread_context, rt_debug_not_in_interrupt,
};
use core::ffi;
use core::pin::Pin;
use core::ptr::{null, null_mut};
use core::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use core::sync::atomic::*;
use kernel::rt_bindings::rt_object;
use kernel::sync::RawSpin;
use kernel::{rt_debug_scheduler_available, rt_object_hook_call};
use pinned_init::*;

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

#[repr(C)]
#[pin_data]
pub struct RtMutex {
    /// Inherit from IPCObject
    #[pin]
    pub parent: IPCObject,
    /// Priority ceiling of mutex
    pub ceiling_priority: ffi::c_uchar,
    /// Maximal priority for pending thread
    pub priority: ffi::c_uchar,
    /// Numbers of thread hold the mutex
    pub hold: ffi::c_uchar,
    /// Reserved field
    pub reserved: ffi::c_uchar,
    /// Current owner of mutex
    #[pin]
    pub owner: *mut RtThread,
    /// The object list taken by thread
    #[pin]
    pub taken_list: ListHead,
    /// Spin lock internal used
    spinlock: RawSpin,
}

impl RtMutex {
    #[inline]
    pub(crate) fn update_priority(&mut self) -> rt_uint8_t {
        unsafe {
            if self.parent.suspend_thread.is_empty() == false {
                if let Some(node) = self.parent.suspend_thread.next() {
                    let thread: *mut RtThread = crate::thread_list_node_entry!(node.as_ptr());
                    self.priority = (*thread).current_priority;
                }
            } else {
                self.priority = 0xff;
            }

            self.priority
        }
    }
}
#[inline]
unsafe extern "C" fn _mutex_update_priority(mutex: *mut RtMutex) -> rt_uint8_t {
    assert!(!mutex.is_null());
    (*mutex).update_priority()
}

/// Get the highest priority inside its taken object and its init priority
#[inline]
unsafe extern "C" fn _thread_get_mutex_priority(thread: *mut RtThread) -> rt_uint8_t {
    assert!(!thread.is_null());
    (*thread).get_mutex_priority()
}

/// Update priority of target thread and the thread suspended it if any
#[inline]
unsafe extern "C" fn _thread_update_priority(
    thread: *mut RtThread,
    priority: ffi::c_uchar,
    suspend_flag: ffi::c_int,
) {
    assert!(!thread.is_null());
    (*thread).update_priority(priority, suspend_flag as u32);
}

#[cfg(feature = "RT_USING_MUTEX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mutex_init(
    mutex: *mut RtMutex,
    name: *const ffi::c_char,
    _flag: ffi::c_uchar,
) -> rt_err_t {
    assert!(!mutex.is_null());

    rt_object_init(
        &mut (*mutex).parent.parent as *mut BaseObject as *mut rt_object,
        ObjectClassType::ObjectClassMutex as rt_object_class_type,
        name,
    );

    _ipc_object_init(&mut (*mutex).parent);

    (*mutex).owner = null_mut();
    (*mutex).priority = 0xFF;
    (*mutex).hold = 0;
    (*mutex).ceiling_priority = 0xFF;

    ListHead::new().__pinned_init(&mut (*mutex).taken_list);

    // Flag can only be RT_IPC_FLAG_PRIO.
    // RT_IPC_FLAG_FIFO cannot solve the unbounded priority inversion problem
    (*mutex).parent.parent.flag = RT_IPC_FLAG_PRIO as rt_uint8_t;

    RT_EOK as rt_err_t
}

#[cfg(feature = "RT_USING_MUTEX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mutex_detach(mutex: *mut RtMutex) -> rt_err_t {
    assert!(!mutex.is_null());
    let obj_ptr = (&mut (*mutex).parent.parent) as *mut BaseObject as *mut rt_object;
    assert_eq!(
        rt_object_get_type(obj_ptr),
        ObjectClassType::ObjectClassMutex as u8
    );
    assert_eq!(rt_object_is_systemobject(obj_ptr), RT_TRUE as i32);

    let level = rt_hw_interrupt_disable();
    // Wakeup all suspended threads
    _ipc_list_resume_all(&mut (*mutex).parent.suspend_thread);
    // Remove mutex from thread's taken list
    Pin::new_unchecked(&mut (*mutex).taken_list).remove();

    rt_hw_interrupt_enable(level);

    Cpu::get_current_scheduler().do_task_schedule();

    // Detach mutex object
    rt_object_detach(obj_ptr);

    RT_EOK as rt_err_t
}

#[cfg(feature = "RT_USING_MUTEX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mutex_drop_thread(mutex: *mut RtMutex, thread: *mut RtThread) {
    let mut priority: rt_uint8_t = 0;
    let mut need_update = false;

    (*thread).remove_tlist();

    if (*mutex).owner.is_null() == false
        && (*(*mutex).owner).current_priority == (*thread).current_priority
    {
        need_update = true;
    }

    // Update the priority of mutex
    if let Some(node) = (*mutex).parent.suspend_thread.next() {
        let thread: *mut RtThread = crate::thread_list_node_entry!(node.as_ptr());
        (*mutex).priority = (*thread).current_priority;
    } else {
        (*mutex).priority = 0xff;
    }

    // Try to change the priority of mutex owner thread
    if need_update == true {
        // Get the maximal priority of mutex in thread
        let owner_thread = (*mutex).owner;
        priority = (*owner_thread).get_mutex_priority();
        if priority != (*owner_thread).current_priority {
            (*owner_thread).update_priority(priority, RT_UNINTERRUPTIBLE as u32);
        }
    }
}

#[cfg(feature = "RT_USING_MUTEX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mutex_setprioceiling(
    mutex: *mut RtMutex,
    priority: ffi::c_uchar,
) -> rt_uint8_t {
    let mut ret_priority: rt_uint8_t = 0xFF;

    if mutex.is_null() == false && priority < RT_THREAD_PRIORITY_MAX as rt_uint8_t {
        //Critical section here if multiple updates to one mutex happen concurrently
        let level = rt_hw_interrupt_disable();
        ret_priority = (*mutex).ceiling_priority;
        (*mutex).ceiling_priority = priority;
        let owner_thread = (*mutex).owner;
        if owner_thread.is_null() == false {
            let priority = (*owner_thread).get_mutex_priority();
            if priority != (*owner_thread).current_priority {
                (*owner_thread).update_priority(priority, RT_UNINTERRUPTIBLE as u32);
            }
        }
        rt_hw_interrupt_enable(level);
    } else {
        rt_set_errno(-(RT_EINVAL as rt_err_t));
    }

    ret_priority
}

#[cfg(feature = "RT_USING_MUTEX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mutex_getprioceiling(mutex: *mut RtMutex) -> rt_uint8_t {
    let mut prio: rt_uint8_t = 0xFF;

    if mutex.is_null() == false {
        prio = (*mutex).ceiling_priority;
    }

    prio
}

#[cfg(all(feature = "RT_USING_MUTEX", feature = "RT_USING_HEAP"))]
#[no_mangle]
pub unsafe extern "C" fn rt_mutex_create(
    name: *const ffi::c_char,
    _flag: ffi::c_uchar,
) -> *mut RtMutex {
    rt_debug_not_in_interrupt!();

    let mutex = rt_object_allocate(
        ObjectClassType::ObjectClassMutex as rt_object_class_type,
        name,
    ) as *mut RtMutex;

    if mutex.is_null() {
        return mutex;
    }

    _ipc_object_init(&mut (*mutex).parent);

    (*mutex).owner = null_mut();
    (*mutex).priority = 0xFF;
    (*mutex).hold = 0;
    (*mutex).ceiling_priority = 0xFF;
    ListHead::new().__pinned_init(&mut (*mutex).taken_list);
    (*mutex).parent.parent.flag = RT_IPC_FLAG_PRIO as rt_uint8_t;

    mutex
}

#[cfg(all(feature = "RT_USING_MUTEX", feature = "RT_USING_HEAP"))]
#[no_mangle]
pub unsafe extern "C" fn rt_mutex_delete(mutex: *mut RtMutex) -> rt_err_t {
    assert!(mutex != null_mut());
    let obj_ptr = &mut (*mutex).parent.parent as *mut BaseObject as *mut rt_object;
    assert_eq!(
        rt_object_get_type(obj_ptr),
        ObjectClassType::ObjectClassMutex as rt_uint8_t
    );
    assert_eq!(rt_object_is_systemobject(obj_ptr), RT_FALSE as rt_bool_t);

    rt_debug_not_in_interrupt!();

    let level = rt_hw_interrupt_disable();
    //Wakeup all suspended threads
    _ipc_list_resume_all(&mut (*mutex).parent.suspend_thread);
    // Remove mutex from thread's taken list
    Pin::new_unchecked(&mut (*mutex).taken_list).remove();
    rt_hw_interrupt_enable(level);

    // Delete mutex object
    rt_object_delete(obj_ptr);

    RT_EOK as rt_err_t
}

#[cfg(feature = "RT_USING_MUTEX")]
#[no_mangle]
unsafe extern "C" fn _rt_mutex_take(
    mutex: *mut RtMutex,
    timeout: ffi::c_int,
    suspend_flag: ffi::c_int,
) -> rt_err_t {
    // Shadow timeout for mutability
    let mut timeout = timeout;
    rt_debug_scheduler_available!(true);
    assert!(!mutex.is_null());
    let obj_ptr = &mut (*mutex).parent.parent as *mut BaseObject as *mut rt_object;
    assert_eq!(
        rt_object_get_type(obj_ptr),
        ObjectClassType::ObjectClassMutex as rt_uint8_t
    );

    let thread = current_thread_ptr!();
    assert!(!thread.is_null());

    let mut level = rt_hw_interrupt_disable();

    rt_object_hook_call!(rt_object_trytake_hook, obj_ptr);

    (*thread).error = RT_EOK as rt_err_t;

    if (*mutex).owner == thread {
        if (*mutex).hold < RT_MUTEX_HOLD_MAX as u8 {
            // Same thread
            (*mutex).hold += 1;
        } else {
            rt_hw_interrupt_enable(level);
            return -(RT_EFULL as rt_err_t);
        }
    } else {
        // Whether the mutex has owner thread.
        if (*mutex).owner.is_null() {
            // Set mutex owner and original priority
            (*mutex).owner = thread;
            (*mutex).priority = 0xff;
            (*mutex).hold = 1;

            if (*mutex).ceiling_priority != 0xFF {
                // Set the priority of thread to the ceiling priority
                if (*mutex).ceiling_priority < (*(*mutex).owner).current_priority {
                    (*(*mutex).owner)
                        .update_priority((*mutex).ceiling_priority, suspend_flag as u32);
                }
            }

            // Insert mutex to thread's taken object list
            Pin::new_unchecked(&mut (*thread).taken_object_list)
                .insert_next(&mut (*mutex).taken_list);
        } else {
            // No waiting, return with timeout
            if timeout == 0 {
                (*thread).error = -(RT_ETIMEOUT as rt_err_t);

                rt_hw_interrupt_enable(level);

                return -(RT_ETIMEOUT as rt_err_t);
            } else {
                let mut priority = (*thread).current_priority;
                // Suspend current thread
                let mut ret = _ipc_list_suspend(
                    &mut (*mutex).parent.suspend_thread,
                    thread,
                    (*mutex).parent.parent.flag,
                    suspend_flag as u32,
                );
                if ret != RT_EOK as rt_err_t {
                    rt_hw_interrupt_enable(level);
                    return ret;
                }

                // Set pending object in thread to this mutex
                (*thread).pending_object =
                    &mut (*mutex).parent.parent as *mut BaseObject as *mut rt_object;

                // Update the priority level of mutex
                if priority < (*mutex).priority {
                    (*mutex).priority = priority;
                    if (*mutex).priority < (*(*mutex).owner).current_priority {
                        (*(*mutex).owner).update_priority(priority, RT_UNINTERRUPTIBLE);
                    }
                }

                if timeout > 0 {
                    rt_timer_control(
                        &mut (*thread).thread_timer,
                        RT_TIMER_CTRL_SET_TIME as i32,
                        (&mut timeout) as *mut i32 as *mut ffi::c_void,
                    );

                    rt_timer_start(&mut (*thread).thread_timer);
                }

                rt_hw_interrupt_enable(level);

                Cpu::get_current_scheduler().do_task_schedule();

                level = rt_hw_interrupt_disable();

                if (*thread).error != RT_EOK as rt_err_t {
                    // The mutex has not been taken and thread has detach from the pending list.
                    let mut need_update = false;

                    if !(*mutex).owner.is_null()
                        && (*(*mutex).owner).current_priority == (*thread).current_priority
                    {
                        need_update = true;
                    }

                    if let Some(node) = (*mutex).parent.suspend_thread.next() {
                        let th = crate::thread_list_node_entry!(node.as_ptr()) as *mut RtThread;
                        (*mutex).priority = (*th).current_priority;
                    } else {
                        (*mutex).priority = 0xff;
                    }

                    // Try to change the priority of mutex owner if necessary
                    if need_update {
                        priority = (*(*mutex).owner).get_mutex_priority();
                        if priority != (*(*mutex).owner).current_priority {
                            (*(*mutex).owner).update_priority(priority, RT_UNINTERRUPTIBLE);
                        }
                    }

                    rt_hw_interrupt_enable(level);

                    (*thread).pending_object = null_mut();

                    ret = (*thread).error;
                    return if ret > 0 { -ret } else { ret };
                }
            }
        }
    }

    rt_hw_interrupt_enable(level);

    rt_object_hook_call!(
        rt_object_take_hook,
        &(*mutex).parent.parent as *const BaseObject as *const rt_object
    );

    RT_EOK as rt_err_t
}

#[cfg(feature = "RT_USING_MUTEX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mutex_take(mutex: *mut RtMutex, time: rt_int32_t) -> rt_err_t {
    _rt_mutex_take(mutex, time, RT_UNINTERRUPTIBLE as ffi::c_int)
}

#[cfg(feature = "RT_USING_MUTEX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mutex_take_interruptible(
    mutex: *mut RtMutex,
    time: rt_int32_t,
) -> rt_err_t {
    _rt_mutex_take(mutex, time, RT_INTERRUPTIBLE as ffi::c_int)
}

#[cfg(feature = "RT_USING_MUTEX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mutex_take_killable(mutex: *mut RtMutex, time: rt_int32_t) -> rt_err_t {
    _rt_mutex_take(mutex, time, RT_KILLABLE as ffi::c_int)
}

#[cfg(feature = "RT_USING_MUTEX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mutex_trytake(mutex: *mut RtMutex) -> rt_err_t {
    rt_mutex_take(mutex, RT_WAITING_NO as ffi::c_int)
}

#[cfg(feature = "RT_USING_MUTEX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mutex_release(mutex: *mut RtMutex) -> rt_err_t {
    assert!(!mutex.is_null());
    assert_eq!(
        rt_object_get_type(&mut (*mutex).parent.parent as *mut BaseObject as *mut rt_object),
        ObjectClassType::ObjectClassMutex as rt_uint8_t
    );

    //Only thread could release mutex because we need test the ownership
    rt_debug_in_thread_context!();

    let thread = current_thread_ptr!();

    let level = rt_hw_interrupt_disable();

    rt_object_hook_call!(
        rt_object_put_hook,
        &mut (*mutex).parent.parent as *mut BaseObject as *mut rt_object
    );

    if thread != (*mutex).owner {
        (*thread).error = -(RT_ERROR as rt_err_t);
        rt_hw_interrupt_enable(level);
        return -(RT_ERROR as rt_err_t);
    }

    (*mutex).hold -= 1;
    let mut need_schedule = false;

    if (*mutex).hold == 0 {
        Pin::new_unchecked(&mut (*mutex).taken_list).remove();

        if (*mutex).ceiling_priority != 0xFF || (*thread).current_priority == (*mutex).priority {
            let mut priority: rt_uint8_t = 0xff;

            priority = (*thread).get_mutex_priority();

            rt_thread_control(
                thread,
                RT_THREAD_CTRL_CHANGE_PRIORITY,
                &mut priority as *mut u8 as *mut ffi::c_void,
            );

            need_schedule = true;
        }

        if !(*mutex).parent.suspend_thread.is_empty() {
            let mut next_thread = null_mut();
            if let Some(node) = (*mutex).parent.suspend_thread.next() {
                next_thread = crate::thread_list_node_entry!(node.as_ptr());
            } else {
                return -(RT_ERROR as rt_err_t);
            }

            Pin::new_unchecked(&mut (*next_thread).tlist).remove();

            (*mutex).owner = next_thread;
            (*mutex).hold = 1;
            Pin::new_unchecked(&mut (*next_thread).taken_object_list)
                .insert_next(&(*mutex).taken_list);

            (*next_thread).pending_object = null_mut();
            (*next_thread).resume();

            if !(*mutex).parent.suspend_thread.is_empty() {
                let mut th = null_mut();
                if let Some(node) = (*mutex).parent.suspend_thread.next() {
                    th = crate::thread_list_node_entry!(node.as_ptr());
                } else {
                    return -(RT_ERROR as rt_err_t);
                }

                (*mutex).priority = (*th).current_priority;
            } else {
                (*mutex).priority = 0xff;
            }

            need_schedule = true;
        } else {
            (*mutex).owner = null_mut();
            (*mutex).priority = 0xff;
        }
    }

    rt_hw_interrupt_enable(level);

    if need_schedule {
        Cpu::get_current_scheduler().do_task_schedule();
    }

    RT_EOK as rt_err_t
}
#[cfg(feature = "RT_USING_MUTEX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mutex_control(
    mutex: *mut RtMutex,
    cmd: ffi::c_int,
    arg: *const ffi::c_void,
) -> rt_err_t {
    assert!(!mutex.is_null());
    assert_eq!(
        rt_object_get_type(&mut (*mutex).parent.parent as *mut BaseObject as *mut rt_object),
        ObjectClassType::ObjectClassMutex as rt_uint8_t
    );

    -(RT_ERROR as rt_err_t)
}
