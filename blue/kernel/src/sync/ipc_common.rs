use crate::impl_kobject;
use crate::list_head_for_each;
use crate::object::*;
use crate::rt_bindings::{
    RT_EOK, RT_ERROR, RT_IPC_FLAG_FIFO, RT_IPC_FLAG_PRIO, RT_THREAD_SUSPEND_MASK,
};
use crate::sync::RawSpin;
use crate::thread::RtThread;
use blue_infra::list::doubly_linked_list::ListHead;
use core::pin::Pin;
use core::ptr::NonNull;
use core::slice;
use pinned_init::*;

/// WaitQueue for IPC
#[repr(C)]
#[pin_data]
pub(crate) struct RtWaitQueue {
    /// WaitQueue impl by ListHead
    #[pin]
    pub(crate) working_queue: ListHead,
    /// WaitQueue working mode, FIFO or PRIO
    waiting_mode: u32,
}

impl RtWaitQueue {
    #[inline]
    pub(crate) fn new(waiting_mode: u32) -> impl PinInit<Self> {
        pin_init!(Self {
            working_queue<-ListHead::new(),
            waiting_mode: waiting_mode
        })
    }

    #[inline]
    pub(crate) fn init(&mut self, waiting_mode: u32) {
        unsafe {
            let _ = ListHead::new().__pinned_init(&mut self.working_queue as *mut ListHead);
        }
        self.waiting_mode = waiting_mode;
    }

    #[inline]
    pub(crate) fn waiting_mode(&self) -> u32 {
        self.waiting_mode
    }

    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.working_queue.is_empty()
    }

    #[inline]
    pub(crate) fn head(&self) -> Option<NonNull<ListHead>> {
        self.working_queue.next()
    }

    #[inline]
    pub(crate) fn tail(&self) -> Option<NonNull<ListHead>> {
        self.working_queue.prev()
    }

    #[inline]
    pub(crate) fn count(&self) -> usize {
        self.working_queue.size()
    }

    pub(crate) fn wait(&mut self, thread: &mut RtThread, pending_mode: u32) -> i32 {
        if (thread.stat as u32 & RT_THREAD_SUSPEND_MASK) != RT_THREAD_SUSPEND_MASK {
            let ret = if thread.suspend(pending_mode) {
                RT_EOK as i32
            } else {
                -(RT_ERROR as i32)
            };

            if ret != RT_EOK as i32 {
                return ret;
            }
        }

        match self.waiting_mode as u32 {
            RT_IPC_FLAG_FIFO => {
                unsafe {
                    Pin::new_unchecked(&mut self.working_queue).insert_prev(&mut thread.tlist)
                };
            }
            RT_IPC_FLAG_PRIO => {
                list_head_for_each!(node, &self.working_queue, {
                    let queued_thread_ptr =
                        unsafe { crate::thread_list_node_entry!(node.as_ptr()) as *mut RtThread };

                    if queued_thread_ptr.is_null() {
                        return -(RT_ERROR as i32);
                    }

                    let queued_thread = unsafe { &mut *queued_thread_ptr };

                    if thread.current_priority < queued_thread.current_priority {
                        let insert_to = unsafe { Pin::new_unchecked(&mut queued_thread.tlist) };
                        insert_to.insert_prev(&mut (thread.tlist));
                    }
                });

                if node.as_ptr() == &self.working_queue as *const ListHead {
                    unsafe {
                        Pin::new_unchecked(&mut self.working_queue).insert_prev(&mut thread.tlist)
                    };
                }
            }
            _ => {
                assert!(false);
            }
        }

        RT_EOK as i32
    }

    #[inline]

    pub(crate) fn wake(&mut self) -> bool {
        if let Some(node) = self.working_queue.next() {
            let thread: *mut RtThread = unsafe { crate::thread_list_node_entry!(node.as_ptr()) };
            if !thread.is_null() {
                unsafe {
                    (*thread).error = RT_EOK as i32;
                }
                return unsafe { (*thread).resume() };
            }
        }

        false
    }

    #[inline]

    pub(crate) fn inner_locked_wake(&mut self) -> bool {
        if let Some(node) = self.working_queue.next() {
            let thread: *mut RtThread = unsafe { crate::thread_list_node_entry!(node.as_ptr()) };
            if !thread.is_null() {
                let spin_lock = RawSpin::new();
                unsafe {
                    (*thread).error = RT_EOK as i32;
                }
                let ret = unsafe { (*thread).resume() };
                spin_lock.unlock();
                return ret;
            }
        }

        false
    }

    #[inline]
    pub(crate) fn wake_all(&mut self) -> bool {
        let mut ret = true;
        while !self.working_queue.is_empty() {
            if let Some(node) = self.working_queue.next() {
                unsafe {
                    let thread: *mut RtThread = crate::thread_list_node_entry!(node.as_ptr());
                    if !thread.is_null() {
                        (*thread).error = -(RT_ERROR as i32);
                        let resume_stat = (*thread).resume();
                        if !resume_stat {
                            ret = resume_stat;
                        }
                    }
                }
            }
        }

        ret
    }

    #[inline]

    pub(crate) fn inner_locked_wake_all(&mut self) -> bool {
        let mut ret = true;
        while !self.working_queue.is_empty() {
            if let Some(node) = self.working_queue.next() {
                let spin_lock = RawSpin::new();
                spin_lock.lock();
                unsafe {
                    let thread: *mut RtThread = crate::thread_list_node_entry!(node.as_ptr());
                    if !thread.is_null() {
                        (*thread).error = -(RT_ERROR as i32);
                        let resume_stat = (*thread).resume();
                        if !resume_stat {
                            ret = resume_stat;
                        }
                    }
                }
                spin_lock.unlock();
            }
        }

        ret
    }
}

/// Base structure of IPC object
#[repr(C)]
#[pin_data]
pub struct IPCObject {
    #[pin]
    /// Inherit from KObjectBase
    pub(crate) parent: KObjectBase,
    /// IPC flag to use
    pub(crate) flag: u8,
    /// Spin lock IPCObject used
    pub(crate) spinlock: RawSpin,
    #[pin]
    /// Threads pended on this IPC object
    pub(crate) wait_list: ListHead,
}

impl_kobject!(IPCObject);

impl IPCObject {
    #[inline]
    pub(crate) fn new(type_: u8, name: [i8; NAME_MAX], flag: u8) -> impl PinInit<Self> {
        pin_init!(Self {
            parent<-KObjectBase::new(type_, name),
            flag: flag,
            spinlock: RawSpin::new(),
            wait_list <- ListHead::new(),
        })
    }

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
    pub(crate) fn reinit(ipcobject: &mut IPCObject) -> i32 {
        unsafe { Pin::new_unchecked(&mut ipcobject.wait_list).reinit() };
        RT_EOK as i32
    }

    pub(crate) fn lock(&mut self) {
        self.spinlock.lock();
    }

    pub(crate) fn unlock(&self) {
        self.spinlock.unlock();
    }

    #[inline]
    pub(crate) fn resume_thread(list: *mut ListHead) -> i32 {
        unsafe {
            if let Some(node) = (*list).next() {
                let thread: *mut RtThread = crate::thread_list_node_entry!(node.as_ptr());
                (*thread).error = RT_EOK as i32;
                (*thread).resume();
            }
        }
        RT_EOK as i32
    }

    #[inline]
    pub(crate) fn resume_all_threads(list: *mut ListHead) -> i32 {
        unsafe {
            while !(*list).is_empty() {
                if let Some(node) = (*list).next() {
                    let spin_lock = RawSpin::new();
                    spin_lock.lock();
                    let thread: *mut RtThread = crate::thread_list_node_entry!(node.as_ptr());
                    (*thread).error = -(RT_ERROR as i32);
                    (*thread).resume();
                    spin_lock.unlock();
                }
            }
        }

        RT_EOK as i32
    }

    pub(crate) fn suspend_thread(
        list: *mut ListHead,
        thread: *mut RtThread,
        flag: u8,
        suspend_flag: u32,
    ) -> i32 {
        unsafe {
            if ((*thread).stat as u32 & RT_THREAD_SUSPEND_MASK) != RT_THREAD_SUSPEND_MASK {
                let ret = if (*thread).suspend(suspend_flag) {
                    RT_EOK as i32
                } else {
                    -(RT_ERROR as i32)
                };

                if ret != RT_EOK as i32 {
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

            RT_EOK as i32
        }
    }

    #[inline]
    pub(crate) fn has_waiting(&self) -> bool {
        !self.wait_list.is_empty()
    }

    #[inline]
    pub(crate) fn wake_one(&mut self) -> i32 {
        Self::resume_thread(&mut self.wait_list)
    }

    #[inline]
    pub(crate) fn wake_all(&mut self) -> i32 {
        Self::resume_all_threads(&mut self.wait_list)
    }

    #[inline]
    pub(crate) fn wait(&mut self, thread: *mut RtThread, flag: u8, suspend_flag: u32) -> i32 {
        Self::suspend_thread(&mut self.wait_list, thread, flag, suspend_flag)
    }
}

pub fn char_ptr_to_array(char_ptr: *const i8) -> [i8; NAME_MAX] {
    // SAFETY: caller should ensure ptr has more mem size than NAME_MAX
    let slice = unsafe { slice::from_raw_parts(char_ptr, NAME_MAX) };

    let mut array: [i8; NAME_MAX] = [0; NAME_MAX];
    array.copy_from_slice(slice);
    array
}
