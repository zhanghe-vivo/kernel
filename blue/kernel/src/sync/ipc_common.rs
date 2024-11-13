use crate::linked_list::ListHead;
use crate::list_head_for_each;
use crate::object::{self, KObjectBase, ObjectClassType};
use crate::rt_bindings::{self, *};
use crate::thread::RtThread;

use crate::impl_kobject;
use crate::str::CStr;
use crate::sync::RawSpin;
use core::ffi;
use core::pin::Pin;
use pinned_init::{pin_data, pin_init_from_closure, PinInit};

/// Base structure of IPC object
#[repr(C)]
#[pin_data]
pub struct IPCObject {
    #[pin]
    /// Inherit from KObjectBase
    pub(crate) parent: KObjectBase,
    /// IPC flag to use
    pub(crate) flag: ffi::c_uchar,
    /// Spin lock IPCObject used
    pub(crate) spinlock: RawSpin,
    #[pin]
    /// Threads pended on this IPC object
    pub(crate) wait_list: ListHead,
}

impl_kobject!(IPCObject);

impl IPCObject {
    #[inline]
    pub(crate) fn init(&mut self, type_: u8, name: *const i8, flag: u8) {
        assert!((flag == RT_IPC_FLAG_FIFO as u8) || (flag == RT_IPC_FLAG_PRIO as u8));
        self.parent.init(type_, name);
        self.flag = flag;
        self.spinlock = RawSpin::new();
        self.init_wait_list();
    }
    #[inline]
    fn init_wait_list(&mut self) {
        unsafe {
            let _ = ListHead::new().__pinned_init(&mut self.wait_list as *mut ListHead);
        }
    }

    #[inline]
    pub(crate) fn new<T>(type_: u8, name: *const i8, flag: u8) -> *mut T {
        assert!((flag == RT_IPC_FLAG_FIFO as u8) || (flag == RT_IPC_FLAG_PRIO as u8));
        unsafe {
            let ipc_obj = KObjectBase::new(type_, name) as *mut IPCObject;
            (*ipc_obj).flag = flag;
            (*ipc_obj).spinlock = RawSpin::new();
            (*ipc_obj).init_wait_list();
            ipc_obj as *mut T
        }
    }
    #[inline]
    pub(crate) fn reinit(ipcobject: &mut IPCObject) -> ffi::c_long {
        unsafe { Pin::new_unchecked(&mut ipcobject.wait_list).reinit() };
        RT_EOK as core::ffi::c_long
    }

    pub(crate) fn lock(&self) {
        self.spinlock.lock();
    }

    pub(crate) fn unlock(&self) {
        self.spinlock.unlock();
    }

    #[inline]
    pub(crate) fn resume_thread(list: *mut ListHead) -> ffi::c_long {
        unsafe {
            if let Some(node) = (*list).next() {
                let thread: *mut RtThread = crate::thread_list_node_entry!(node.as_ptr());
                (*thread).error = RT_EOK as ffi::c_int;
                (*thread).resume();
            }
        }
        RT_EOK as ffi::c_long
    }

    #[inline]
    pub(crate) fn resume_all_threads(list: *mut ListHead) -> ffi::c_long {
        unsafe {
            while !(*list).is_empty() {
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

        RT_EOK as ffi::c_long
    }

    pub(crate) fn suspend_thread(
        list: *mut ListHead,
        thread: *mut RtThread,
        flag: rt_uint8_t,
        suspend_flag: u32,
    ) -> ffi::c_long {
        unsafe {
            if ((*thread).stat as u32 & RT_THREAD_SUSPEND_MASK) != RT_THREAD_SUSPEND_MASK {
                let ret = if (*thread).suspend(suspend_flag) {
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
                        let s_thread =
                            crate::thread_list_node_entry!(node.as_ptr()) as *mut RtThread;
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

            RT_EOK as ffi::c_long
        }
    }

    #[inline]
    pub(crate) fn has_waiting(&self) -> bool {
        !self.wait_list.is_empty()
    }

    #[inline]
    pub(crate) fn wake_one(&mut self) -> ffi::c_long {
        Self::resume_thread(&mut self.wait_list)
    }

    #[inline]
    pub(crate) fn wake_all(&mut self) -> ffi::c_long {
        Self::resume_all_threads(&mut self.wait_list)
    }

    #[inline]
    pub(crate) fn wait(
        &mut self,
        thread: *mut RtThread,
        flag: ffi::c_uchar,
        suspend_flag: u32,
    ) -> ffi::c_long {
        Self::suspend_thread(&mut self.wait_list, thread, flag, suspend_flag)
    }
}

#[no_mangle]
pub extern "C" fn _ipc_object_init(list: &mut IPCObject) {
    unsafe {
        let _ = ListHead::new().__pinned_init(&mut list.wait_list as *mut ListHead);
    }
}
