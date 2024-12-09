use crate::allocator::{align_up_size, rt_free, rt_malloc};
use crate::cpu::Cpu;
use crate::error::{code, Error};
use crate::impl_kobject;
use crate::klibc::rt_memcpy;
use crate::list_head_for_each;
use crate::object::*;
use crate::rt_bindings::{
    RT_EFULL, RT_EOK, RT_ERROR, RT_IPC_FLAG_FIFO, RT_IPC_FLAG_PRIO, RT_MQ_ENTRY_MAX,
    RT_THREAD_SUSPEND_MASK,
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

pub(crate) const IPC_SYS_QUEUE_FIFO: u32 = 0;
pub(crate) const IPC_SYS_QUEUE_PRIO: u32 = 1;
pub(crate) const IPC_SYS_QUEUE_STUB: u32 = 2;

macro_rules! sys_queue_item_data_addr {
    ($addr:expr) => {
        ($addr as *mut RtSysQueueItemHeader).offset(1) as *mut _
    };
}

/// Sys Queue item header
#[repr(C)]
pub(crate) struct RtSysQueueItemHeader {
    pub(crate) next: *mut RtSysQueueItemHeader,
    pub(crate) len: usize,
    pub(crate) prio: i32,
}

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
    /// Ringbuffer read position
    pub(crate) read_pos: usize,
    /// Ringbuffer write position
    pub(crate) write_pos: usize,
    /// Queue head pointer
    pub(crate) head: Option<NonNull<u8>>,
    /// Queue tail pointer
    pub(crate) tail: Option<NonNull<u8>>,
    /// Pointer to first 'free to use' item in queue
    pub(crate) free: Option<NonNull<u8>>,
    /// Queue working mode: FIFO by default
    pub(crate) working_mode: u32,
    /// Queue for waiting to enqueue items in the working sysqueue
    #[pin]
    pub(crate) enqueue_waiter: RtWaitQueue,
    /// Queue for waiting to dequeue items in the working sysqueue
    #[pin]
    pub(crate) dequeue_waiter: RtWaitQueue,
    /// Spin lock for sysqueue
    pub(crate) spinlock: RawSpin,
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
        } else {
            self.is_storage_from_external = true;
        }

        rt_bindings::rt_debug_not_in_interrupt!();
        let mut item_align_size = 0;
        if self.working_mode == 0 {
            self.queue_buf_size = self.item_size * self.item_max_count;
        } else {
            item_align_size = align_up_size(self.item_size, rt_bindings::RT_ALIGN_SIZE as usize);
            self.queue_buf_size =
                (item_align_size + mem::size_of::<RtSysQueueItemHeader>()) * self.item_max_count;
        }

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

        if self.working_mode == IPC_SYS_QUEUE_FIFO {
            self.free = None;
            self.head = None;
            self.tail = None;
        } else if self.working_mode == IPC_SYS_QUEUE_PRIO {
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
        }

        self.queue_buf_size
    }

    pub(crate) fn free_storage_internal(&mut self) {
        if let Some(mut buffer) = self.queue_buf {
            if !self.is_storage_from_external {
                unsafe {
                    rt_free(buffer.as_mut() as *mut u8 as *mut ffi::c_void);
                }
                self.queue_buf = None;
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
            sysq.item_in_queue = 0;
            sysq.read_pos = 0;
            sysq.write_pos = 0;
            sysq.queue_buf = None;
            sysq.init_storage_internal(null_mut());
            sysq.working_mode = working_mode;

            unsafe {
                RtWaitQueue::new(waiting_mode)
                    .__pinned_init(&mut sysq.enqueue_waiter as *mut RtWaitQueue);
                RtWaitQueue::new(waiting_mode)
                    .__pinned_init(&mut sysq.dequeue_waiter as *mut RtWaitQueue);
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
        self.item_in_queue = 0;
        self.read_pos = 0;
        self.write_pos = 0;
        let buf_size = self.init_storage_internal(buffer);
        self.working_mode = working_mode;

        unsafe {
            RtWaitQueue::new(waiting_mode)
                .__pinned_init(&mut self.enqueue_waiter as *mut RtWaitQueue);
            RtWaitQueue::new(waiting_mode)
                .__pinned_init(&mut self.dequeue_waiter as *mut RtWaitQueue);
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
    pub(crate) fn count(&self) -> usize {
        self.item_in_queue
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

    #[inline]
    pub(crate) fn push_fifo(&mut self, buffer: *const u8, size: usize) -> i32 {
        assert_eq!(self.item_size, size);

        if let Some(mut buffer_raw) = self.queue_buf {
            unsafe {
                rt_memcpy(
                    buffer_raw.as_ptr().offset(self.write_pos as isize) as *mut core::ffi::c_void,
                    buffer as *const core::ffi::c_void,
                    size as usize,
                );
            }
        } else {
            return 0;
        }

        self.write_pos += self.item_size;
        if self.write_pos >= self.queue_buf_size {
            self.write_pos = 0;
        }

        if self.item_in_queue < rt_bindings::RT_MB_ENTRY_MAX as usize {
            self.item_in_queue += 1;
            return size as i32;
        } else {
            return 0;
        }
    }

    #[inline]
    pub(crate) fn pop_fifo(&mut self, buffer: &mut *mut u8, size: usize) -> i32 {
        assert_eq!(self.item_size, size);

        if let Some(buffer_raw) = self.queue_buf {
            unsafe {
                rt_memcpy(
                    *buffer as *mut core::ffi::c_void,
                    buffer_raw.as_ptr().offset(self.read_pos as isize) as *const core::ffi::c_void,
                    size as usize,
                );
            }
        } else {
            return 0;
        }

        self.read_pos += self.item_size;
        if self.read_pos >= self.queue_buf_size {
            self.read_pos = 0;
        }

        if self.item_in_queue > 0 {
            self.item_in_queue -= 1;
        }

        size as i32
    }

    #[inline]
    pub(crate) fn urgent_fifo(&mut self, buffer: *const u8, size: usize) -> i32 {
        if self.read_pos > 0 {
            self.read_pos -= self.item_size;
        } else {
            self.read_pos = self.queue_buf_size - self.item_size;
        }

        if let Some(buffer_raw) = self.queue_buf {
            unsafe {
                rt_memcpy(
                    buffer_raw.as_ptr().offset(self.read_pos as isize) as *mut core::ffi::c_void,
                    buffer as *const core::ffi::c_void,
                    size as usize,
                );
            }
        } else {
            return 0;
        }

        self.item_in_queue += 1;
        size as i32
    }

    pub(crate) fn push_prio(&mut self, buffer: *const u8, size: usize, priority: i32) -> i32 {
        assert!(!self.free.is_none());

        let mut hdr = self.free.unwrap().as_ptr() as *mut RtSysQueueItemHeader;
        let mut hdr_ref = unsafe { &mut *hdr };

        self.free = unsafe { Some(NonNull::new_unchecked(hdr_ref.next as *mut u8)) };

        self.spinlock.unlock();

        hdr_ref.next = null_mut();

        hdr_ref.len = size;

        unsafe {
            rt_memcpy(
                sys_queue_item_data_addr!(hdr),
                buffer as *const core::ffi::c_void,
                size as usize,
            );
        }

        self.spinlock.lock();

        hdr_ref.prio = priority;
        if self.head.is_none() {
            self.head = unsafe { Some(NonNull::new_unchecked(hdr as *mut u8)) };
        }

        let mut node = self.head.unwrap().as_ptr() as *mut RtSysQueueItemHeader;
        let mut prev_node: *mut RtSysQueueItemHeader = null_mut();

        while !node.is_null() {
            if unsafe { (*node).prio < (*hdr).prio } {
                if (prev_node == null_mut()) {
                    self.head = unsafe { Some(NonNull::new_unchecked(hdr as *mut u8)) };
                } else {
                    unsafe { (*prev_node).next = hdr };
                }

                unsafe {
                    (*hdr).next = node;
                }
                break;
            }

            if unsafe { (*node).next == null_mut() } {
                if node != hdr {
                    unsafe { (*node).next = hdr };
                }
                self.tail = unsafe { Some(NonNull::new_unchecked(hdr as *mut u8)) };
                break;
            }
            prev_node = node;
            unsafe { node = (*node).next };
        }

        if self.item_in_queue < rt_bindings::RT_MQ_ENTRY_MAX as usize {
            // increase message entry
            self.item_in_queue += 1;
        } else {
            self.spinlock.unlock();
            return -(RT_EFULL as i32);
        }

        size as i32
    }

    #[inline]
    pub(crate) fn urgent_prio(&mut self, buffer: *const u8, size: usize) -> i32 {
        self.spinlock.lock();

        let mut hdr = null_mut();
        if self.free.is_some() {
            hdr = self.free.unwrap().as_ptr() as *mut RtSysQueueItemHeader;
        }

        if hdr.is_null() {
            self.spinlock.unlock();
            return -(RT_EFULL as i32);
        }

        // SAFETY: msg is null checked and buffer is valid
        self.free = unsafe { Some(NonNull::new_unchecked((*hdr).next as *mut u8)) };

        self.spinlock.unlock();

        unsafe { (*hdr).len = size };

        unsafe {
            rt_memcpy(
                sys_queue_item_data_addr!(hdr),
                buffer as *const core::ffi::c_void,
                size as usize,
            );
        }

        self.spinlock.lock();

        unsafe { (*hdr).next = self.head.unwrap().as_ptr() as *mut RtSysQueueItemHeader };

        self.head = unsafe { Some(NonNull::new_unchecked(hdr as *mut u8)) };

        if self.tail.is_none() {
            self.tail = unsafe { Some(NonNull::new_unchecked(hdr as *mut u8)) };
        }

        if self.item_in_queue < RT_MQ_ENTRY_MAX as usize {
            self.item_in_queue += 1;
        } else {
            self.spinlock.unlock();
            return -(RT_EFULL as i32);
        }

        if !self.dequeue_waiter.is_empty() {
            self.dequeue_waiter.wake();

            self.spinlock.unlock();

            Cpu::get_current_scheduler().do_task_schedule();

            return RT_EOK as i32;
        }

        self.spinlock.unlock();

        size as i32
    }

    #[inline]
    pub(crate) fn pop_prio(&mut self, buffer: *mut u8, size: usize, prio: *mut i32) -> i32 {
        let hdr = self.head.unwrap().as_ptr() as *mut RtSysQueueItemHeader;

        // SAFETY: msg is null checked
        unsafe { self.head = Some(NonNull::new_unchecked((*hdr).next as *mut u8)) };

        if self.tail.unwrap().as_ptr() == hdr as *mut u8 {
            self.tail = None;
        }

        if self.item_in_queue > 0 {
            self.item_in_queue -= 1;
        }

        self.spinlock.unlock();

        // SAFETY: msg is null checked
        let mut len = unsafe { (*hdr).len as usize };

        if len > size {
            len = size;
        }

        // SAFETY: hdr is null checked and buffer is valid
        unsafe {
            rt_memcpy(
                buffer as *mut core::ffi::c_void,
                sys_queue_item_data_addr!(hdr),
                len,
            )
        };

        if !prio.is_null() {
            //SAFETY: msg is null checked and prio is valid
            unsafe { *prio = (*hdr).prio };
        }

        self.spinlock.lock();

        // SAFETY: msg is null checked
        unsafe { (*hdr).next = self.free.unwrap().as_ptr() as *mut RtSysQueueItemHeader };

        self.free = unsafe { Some(NonNull::new_unchecked(hdr as *mut u8)) };

        if !self.enqueue_waiter.is_empty() {
            self.enqueue_waiter.wake();
            self.spinlock.unlock();

            Cpu::get_current_scheduler().do_task_schedule();

            return len as i32;
        }

        self.spinlock.unlock();

        size as i32
    }

    #[inline]
    pub(crate) fn lock(&self) {
        self.spinlock.lock();
    }
    #[inline]
    pub(crate) fn unlock(&self) {
        self.spinlock.unlock();
    }
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
