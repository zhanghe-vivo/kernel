use crate::linked_list::ListHead;
use crate::object::{self, BaseObject, ObjectClassType};
use crate::rt_bindings::{self, *};
use crate::thread::RtThread;
use crate::{list_head_for_each, rt_list_entry, rt_list_init};

use crate::str::CStr;
use crate::sync::RawSpin;
use core::ffi;
use core::pin::Pin;
use pinned_init::{pin_data, pin_init_from_closure, PinInit};

pub type IpcFlagType = i32;

#[macro_export]
macro_rules! rt_get_message_addr {
    ($msg:expr) => {
        ($msg as *mut rt_mq_message).offset(1) as *mut _
    };
}

/// Base structure of IPC object
#[repr(C)]
#[derive(Debug)]
#[pin_data]
pub struct IPCObject {
    #[pin]
    /// inherit from BaseObject
    pub parent: BaseObject,
    #[pin]
    /// threads pended on this resource
    pub suspend_thread: ListHead,
}

impl IPCObject {
    #[inline]
    pub fn new(
        name: &'static CStr,
        obj_type: ObjectClassType,
        flag: IpcFlagType,
        is_static: bool,
    ) -> impl PinInit<Self> {
        let init = move |slot: *mut Self| unsafe {
            assert!(
                (flag == RT_IPC_FLAG_FIFO as IpcFlagType)
                    || (flag == RT_IPC_FLAG_PRIO as IpcFlagType)
            );
            if is_static {
                object::rt_object_init(
                    &mut (*slot).parent as *mut BaseObject as *mut rt_bindings::rt_object,
                    obj_type as u32,
                    name.as_char_ptr(),
                )
            } else {
                object::rt_object_init_dyn(
                    &mut (*slot).parent as *mut BaseObject as *mut rt_bindings::rt_object,
                    obj_type as u32,
                    name.as_char_ptr(),
                )
            }

            let cur_ref = &mut *slot;
            let _ = ListHead::new().__pinned_init(&mut cur_ref.suspend_thread as *mut ListHead);
            Ok(())
        };
        unsafe { pin_init_from_closure(init) }
    }

    pub fn reinit(ipcobject: &mut IPCObject) -> rt_err_t {
        unsafe { Pin::new_unchecked(&mut ipcobject.suspend_thread).reinit() };
        RT_EOK as rt_err_t
    }
}

#[no_mangle]
pub extern "C" fn _ipc_object_init(object: &mut IPCObject) -> rt_err_t {
    unsafe {
        let _ = ListHead::new().__pinned_init(&mut object.suspend_thread as *mut ListHead);
    }

    RT_EOK as rt_err_t
}

#[no_mangle]
pub extern "C" fn _ipc_list_resume(list: *mut ListHead) -> rt_err_t {
    unsafe {
        if let Some(node) = (*list).next() {
            let thread: *mut RtThread = crate::thread_list_node_entry!(node.as_ptr());
            (*thread).error = RT_EOK as rt_err_t;
            (*thread).resume();
        }
    }
    RT_EOK as rt_err_t
}

#[no_mangle]
pub extern "C" fn _ipc_list_resume_all(list: *mut ListHead) -> rt_err_t {
    unsafe {
        while (*list).is_empty() == false {
            if let Some(node) = (*list).next() {
                let spin_lock = RawSpin::new();
                spin_lock.lock();
                let thread: *mut RtThread = crate::thread_list_node_entry!(node.as_ptr());
                (*thread).error = -(RT_ERROR as rt_err_t);
                (*thread).resume();
                spin_lock.unlock();
            }
        }
    }

    RT_EOK as rt_err_t
}

#[no_mangle]
pub extern "C" fn _ipc_list_suspend(
    list: *mut ListHead,
    thread: *mut RtThread,
    flag: rt_uint8_t,
    suspend_flag: u32,
) -> rt_err_t {
    unsafe {
        if ((*thread).stat as u32 & RT_THREAD_SUSPEND_MASK) != RT_THREAD_SUSPEND_MASK {
            let ret = if (*thread).suspend(suspend_flag) == true {
                RT_EOK as rt_err_t
            } else {
                -(RT_ERROR as rt_err_t)
            };

            if ret != RT_EOK as rt_err_t {
                return ret;
            }
        }

        match flag as u32 {
            RT_IPC_FLAG_FIFO => {
                Pin::new_unchecked(&mut *list).insert_prev(&mut (*thread).tlist);
            }
            RT_IPC_FLAG_PRIO => {
                list_head_for_each!(node, &(*list), {
                    let s_thread = crate::thread_list_node_entry!(node.as_ptr()) as *mut RtThread;
                    if (*thread).current_priority < (*s_thread).current_priority {
                        let insert_to = Pin::new_unchecked(&mut ((*s_thread).tlist));
                        insert_to.insert_prev(&mut ((*thread).tlist));
                    }
                });

                if node.as_ptr() == list {
                    Pin::new_unchecked(&mut *list).insert_prev(&mut (*thread).tlist);
                }
            }
            _ => {
                assert!(false);
            }
        }

        RT_EOK as rt_err_t
    }
}

#[no_mangle]
pub unsafe extern "C" fn _rt_memcpy(
    dst: *mut ffi::c_void,
    src: *const ffi::c_void,
    size: usize,
) -> *mut ffi::c_void {
    dst.copy_from(src, size);
    dst
}
