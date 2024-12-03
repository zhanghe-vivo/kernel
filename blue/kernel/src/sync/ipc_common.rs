use crate::allocator::{align_up_size, rt_free, rt_malloc};
use crate::error::{code, Error};
use crate::impl_kobject;
use crate::list_head_for_each;
use crate::object::*;
use crate::rt_bindings::{
    RT_EOK, RT_ERROR, RT_IPC_FLAG_FIFO, RT_IPC_FLAG_PRIO, RT_THREAD_SUSPEND_MASK,
};
use crate::sync::RawSpin;
use crate::thread::{RtThread, SuspendFlag};
use blue_infra::list::doubly_linked_list::ListHead;
use core::ffi;
use core::mem;
use core::pin::Pin;
use core::ptr::{null_mut, NonNull};
use core::slice;
use pinned_init::*;

/// System queue for kernel use on IPC
#[repr(C)]
#[pin_data(PinnedDrop)]
pub(crate) struct RtSysQueue {
    /// Queue item size
    pub(crate) item_size: usize,
    /// Queue item max count
    pub(crate) item_max_count: usize,
    /// Count of items in queue
    pub(crate) item_in_queue: usize,
    /// Queue raw buffer pointer
    pub(crate) queue_buf: Option<NonNull<u8>>,
    /// Queue memory size
    pub(crate) queue_buf_size: usize,
    /// If the queue buffer from external, this will be true
    is_storage_from_external: bool,
    /// Queue head pointer
    pub(crate) head: Option<NonNull<u8>>,
    /// Queue tail pointer
    pub(crate) tail: Option<NonNull<u8>>,
    /// Pointer to first 'free to use' item in queue
    pub(crate) free: Option<NonNull<u8>>,
    /// Queue working mode: FIFO by default
    pub(crate) working_mode: u32,
    /// Queue for waiting to send items
    #[pin]
    pub(crate) sender: RtWaitQueue,
    /// Queue for waiting to receive items
    #[pin]
    pub(crate) receiver: RtWaitQueue,
    /// Spin lock for queue
    spinlock: RawSpin,
}

#[pinned_drop]
impl PinnedDrop for RtSysQueue {
    fn drop(self: Pin<&mut Self>) {
        let queue_raw = unsafe { Pin::get_unchecked_mut(self) };
        queue_raw.free_storage_internal();
    }
}

impl RtSysQueue {
    fn init_storage_internal(&mut self, raw_buf_ptr: *mut u8) -> usize {
        if self.item_size == 0 || self.item_max_count == 0 || self.working_mode == 2 {
            return 0;
        }

        self.free_storage_internal();

        if raw_buf_ptr.is_null() {
            self.is_storage_from_external = false;
        }

        rt_bindings::rt_debug_not_in_interrupt!();

        let item_align_size = align_up_size(self.item_size, rt_bindings::RT_ALIGN_SIZE as usize);
        self.queue_buf_size =
            (item_align_size + mem::size_of::<RtSysQueueItemHeader>()) * self.item_max_count;
        let buffer_raw = if self.is_storage_from_external {
            raw_buf_ptr
        } else {
            // SAFETY: return null pointer when failed
            unsafe { rt_malloc(self.queue_buf_size) as *mut u8 }
        };

        if buffer_raw.is_null() {
            return 0;
        } else {
            // SAFETY: buffer_raw is null checked and allocated to the proper size
            self.queue_buf = Some(unsafe { NonNull::new_unchecked(buffer_raw) });
        }

        let mut free_raw = null_mut();
        for idx in 0..self.item_max_count {
            // SAFETY: buffer_raw is null checked and allocated to the proper size
            let head_raw = unsafe {
                buffer_raw.offset(
                    (idx * (item_align_size + mem::size_of::<RtSysQueueItemHeader>())) as isize,
                ) as *mut RtSysQueueItemHeader
            };

            if head_raw.is_null() {
                // SAFETY: header_raw is null checked and within the range
                unsafe { (*head_raw).next = free_raw as *mut RtSysQueueItemHeader };
            }

            free_raw = head_raw;
        }

        // SAFETY: free_raw is null checked and within the range
        self.free = if free_raw.is_null() {
            None
        } else {
            Some(unsafe { NonNull::new_unchecked(free_raw as *mut u8) })
        };
        self.head = None;
        self.tail = None;

        self.queue_buf_size
    }

    fn free_storage_internal(&mut self) {
        if let Some(mut buffer) = self.queue_buf {
            if !self.is_storage_from_external {
                unsafe {
                    rt_free(buffer.as_mut() as *mut u8 as *mut ffi::c_void);
                }
            }
        }
    }

    #[inline]
    pub(crate) fn new(
        item_size: usize,
        item_max_count: usize,
        working_mode: u32,
        waiting_mode: u32,
    ) -> impl PinInit<Self> {
        let init = move |slot: *mut Self| {
            let sysq = unsafe { &mut *(slot as *mut RtSysQueue) };
            sysq.item_size = item_size;
            sysq.item_max_count = item_max_count;
            sysq.init_storage_internal(null_mut());
            sysq.working_mode = working_mode;

            unsafe {
                RtWaitQueue::new(waiting_mode).__pinned_init(&mut sysq.sender as *mut RtWaitQueue);
                RtWaitQueue::new(waiting_mode)
                    .__pinned_init(&mut sysq.receiver as *mut RtWaitQueue);
            }

            sysq.spinlock = RawSpin::new();

            Ok(())
        };
        unsafe { pin_init_from_closure(init) }
    }

    #[inline]
    pub fn init(
        &mut self,
        buffer: *mut u8,
        item_size: usize,
        item_max_count: usize,
        working_mode: u32,
        waiting_mode: u32,
    ) -> Error {
        self.item_size = item_size;
        self.item_max_count = item_max_count;
        let buf_size = self.init_storage_internal(buffer);
        self.working_mode = working_mode;

        unsafe {
            RtWaitQueue::new(waiting_mode).__pinned_init(&mut self.sender as *mut RtWaitQueue);
            RtWaitQueue::new(waiting_mode).__pinned_init(&mut self.receiver as *mut RtWaitQueue);
        }

        self.spinlock = RawSpin::new();

        if buf_size > 0 {
            code::EOK
        } else {
            code::ENOMEM
        }
    }

    #[inline]
    pub(crate) fn is_full(&self) -> bool {
        self.item_max_count == self.item_in_queue
    }

    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.item_in_queue == 0
    }

    #[inline]
    pub(crate) fn force_push_stub(&mut self) -> bool {
        if self.item_in_queue < core::usize::MAX {
            self.item_in_queue += 1;
            return true;
        }
        false
    }

    #[inline]
    pub(crate) fn push_stub(&mut self) -> bool {
        if self.is_full() {
            return false;
        }
        self.force_push_stub()
    }

    #[inline]
    pub(crate) fn pop_stub(&mut self) -> bool {
        if self.is_empty() {
            return false;
        }
        self.item_in_queue -= 1;
        true
    }

    #[inline]
    pub(crate) fn reset_stub(&mut self, item_in_queue: usize) {
        self.item_in_queue = item_in_queue;
        if self.item_in_queue > self.item_max_count {
            self.item_max_count = self.item_in_queue;
        }
    }
    /*
        #[inline]
        pub(crate) fn push_internal(
            &mut self,
            buffer: *const u8,
            size: usize,
            prio: i32,
            time_out: i32,
            pending_mode: u32,
        ) {
        }
    */
    #[inline]
    pub(crate) fn pop_internal(&self) /*-> Result<RtSysQueueItemHeader, Error>*/
    {
        //self.head.is_none()
    }

    #[inline]
    pub(crate) fn lock(&self) {
        self.spinlock.lock();
    }
    #[inline]
    pub(crate) fn unlock(&self) {
        self.spinlock.unlock();
    }
    #[inline]
    pub(crate) fn push_internal(&mut self, item: *mut u8) -> Result<(), Error> {
        Ok(()) //self.push_internal(item)
    }
}

/// Sys Queue item header
#[repr(C)]
pub(crate) struct RtSysQueueItemHeader {
    pub(crate) next: *mut RtSysQueueItemHeader,
    pub(crate) len: usize,
    pub(crate) priority: i32,
}

#[repr(C)]
pub(crate) struct RtSysQueueItemHandleBorrowed {
    pub(crate) addr: *mut u8,
    pub(crate) len: usize,
}

macro_rules! sys_queue_item_data_addr {
    ($msg:expr) => {
        ($msg as *mut RtSysQueueItemHeader).offset(1) as *mut _
    };
}

/// WaitQueue for pending threads
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
        if !thread.stat.is_suspended() {
            let ret = if thread.suspend(SuspendFlag::from_u8(pending_mode as u8)) {
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

                    if thread.priority.get_current() < queued_thread.priority.get_current() {
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
            _ => {}
        }

        RT_EOK as i32
    }

    #[inline]

    pub(crate) fn wake(&mut self) -> bool {
        if let Some(node) = self.working_queue.next() {
            let thread: *mut RtThread = unsafe { crate::thread_list_node_entry!(node.as_ptr()) };
            if !thread.is_null() {
                unsafe {
                    (*thread).error = code::EOK;
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
                    (*thread).error = code::EOK;
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
                        (*thread).error = code::ERROR;
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
                        (*thread).error = code::ERROR;
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
                (*thread).error = code::EOK;
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
                    (*thread).error = code::ERROR;
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
            if !(*thread).stat.is_suspended() {
                let ret = if (*thread).suspend(SuspendFlag::from_u8(suspend_flag as u8)) {
                    RT_EOK as i32
                } else {
                    -(RT_ERROR as i32)
                };
            }

            match flag as u32 {
                RT_IPC_FLAG_FIFO => {
                    Pin::new_unchecked(&mut *list).insert_prev(&mut (*thread).tlist);
                }
                RT_IPC_FLAG_PRIO => {
                    list_head_for_each!(node, &(*list), {
                        let s_thread =
                            crate::thread_list_node_entry!(node.as_ptr()) as *mut RtThread;
                        if (*thread).priority.get_current() < (*s_thread).priority.get_current() {
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
