use crate::{
    allocator::{align_up_size, rt_free, rt_malloc},
    clock::rt_tick_get,
    cpu::Cpu,
    error::Error,
    impl_kobject,
    klibc::rt_memcpy,
    list_head_for_each,
    object::*,
    print, println,
    rt_bindings::{
        rt_debug_not_in_interrupt, rt_debug_scheduler_available, rt_err_t, rt_int32_t, rt_object,
        rt_object_hook_call, rt_size_t, rt_ssize_t, rt_ubase_t, rt_uint32_t, rt_uint8_t,
        RT_ALIGN_SIZE, RT_EFULL, RT_EINTR, RT_EINVAL, RT_ENOMEM, RT_EOK, RT_ERROR, RT_ETIMEOUT,
        RT_INTERRUPTIBLE, RT_IPC_CMD_RESET, RT_IPC_FLAG_FIFO, RT_IPC_FLAG_PRIO, RT_KILLABLE,
        RT_MQ_ENTRY_MAX, RT_TIMER_CTRL_SET_TIME, RT_UNINTERRUPTIBLE,
    },
    sync::ipc_common::*,
    thread::RtThread,
};
use blue_infra::list::doubly_linked_list::ListHead;
#[allow(unused_imports)]
use core::{
    alloc::AllocError,
    cell::UnsafeCell,
    ffi::{self, c_char, c_void},
    marker::PhantomPinned,
    mem,
    mem::MaybeUninit,
    ptr::null_mut,
    slice,
};

use crate::alloc::boxed::Box;
use core::pin::Pin;
use kernel::{fmt, str::CString};

use cfg_if;
use pinned_init::*;

#[pin_data(PinnedDrop)]
pub struct KMessageQueue {
    #[pin]
    raw: UnsafeCell<RtMessageQueue>,
    #[pin]
    pin: PhantomPinned,
}

unsafe impl Send for KMessageQueue {}
unsafe impl Sync for KMessageQueue {}

#[pinned_drop]
impl PinnedDrop for KMessageQueue {
    fn drop(self: Pin<&mut Self>) {
        unsafe {
            (*self.raw.get()).detach();
        }
    }
}

impl KMessageQueue {
    pub fn new(msg_size: usize, max_msgs: usize) -> Pin<Box<Self>> {
        fn init_raw(msg_size: usize, max_msgs: usize) -> impl PinInit<UnsafeCell<RtMessageQueue>> {
            let init = move |slot: *mut UnsafeCell<RtMessageQueue>| {
                let slot: *mut RtMessageQueue = slot.cast();
                unsafe {
                    let cur_ref = &mut *slot;

                    if let Ok(s) = CString::try_from_fmt(fmt!("{:p}", slot)) {
                        cur_ref.init_new_storage(
                            s.as_ptr() as *const i8,
                            msg_size as u16,
                            max_msgs as u16,
                            RT_IPC_FLAG_PRIO as u8,
                        );
                    } else {
                        let default = "default";
                        cur_ref.init_new_storage(
                            default.as_ptr() as *const i8,
                            msg_size as u16,
                            max_msgs as u16,
                            RT_IPC_FLAG_PRIO as u8,
                        );
                    }
                }
                Ok(())
            };
            unsafe { pin_init_from_closure(init) }
        }

        Box::pin_init(pin_init!(Self {
            raw<-init_raw(msg_size, max_msgs),
            pin: PhantomPinned,
        }))
        .unwrap()
    }

    pub fn send(&self, msg: &[u8]) -> Result<(), Error> {
        let msg_ptr: *const u8 = msg.as_ptr();
        let size: usize = msg.len();

        let result = unsafe { (*self.raw.get()).send(msg_ptr, size) };
        if result == RT_EOK as rt_err_t {
            Ok(())
        } else {
            Err(Error::from_errno(result))
        }
    }

    pub fn receive(&self, timeout: i32) -> Result<Box<[u8]>, Error> {
        let mut buffer = null_mut();
        let mut size = 0 as usize;
        let result = unsafe { (*self.raw.get()).receive(buffer, size, timeout) };
        if result == RT_EOK as i32 {
            if buffer.is_null() || size == 0 {
                return Ok(Box::new([]));
            }
            unsafe {
                let slice_ptr = slice::from_raw_parts_mut(buffer, size);
                Ok(Box::from_raw(slice_ptr))
            }
        } else {
            Err(Error::from_errno(result))
        }
    }
}

macro_rules! get_message_addr {
    ($msg:expr) => {
        ($msg as *mut RtMessage).offset(1) as *mut _
    };
}

/// MessageQueue message structure
#[repr(C)]
#[pin_data]
pub struct RtMessage {
    #[pin]
    pub next: *mut RtMessage,
    pub length: isize,
    pub prio: i32,
}

/// MessageQueue raw structure
#[repr(C)]
#[pin_data(PinnedDrop)]
pub struct RtMessageQueue {
    /// Inherit from IPCObject
    #[pin]
    pub(crate) parent: IPCObject,
    /// Start address of message queue
    pub(crate) msg_pool: *mut u8,
    /// Message size of each message
    pub(crate) msg_size: u16,
    /// Max number of messages
    pub(crate) max_msgs: u16,
    /// Index of messages in the queue
    pub(crate) entry: u16,
    /// List head
    pub(crate) msg_queue_head: *mut u8,
    /// List tail
    pub(crate) msg_queue_tail: *mut u8,
    /// Pointer indicated the free node of queue
    pub(crate) msg_queue_free: *mut u8,
    /// Sender thread suspended on this message queue
    #[pin]
    pub(crate) suspend_sender_thread: ListHead,
}

impl_kobject!(RtMessageQueue);

#[pinned_drop]
impl PinnedDrop for RtMessageQueue {
    fn drop(self: Pin<&mut Self>) {
        let mq_raw = unsafe { Pin::get_unchecked_mut(self) };

        if !mq_raw.msg_pool.is_null() {
            unsafe {
                rt_free(mq_raw.msg_pool as *mut ffi::c_void);
            }
        }
    }
}

impl RtMessageQueue {
    pub fn new(
        name: [i8; NAME_MAX],
        msg_size: u16,
        max_msgs: u16,
        flag: u8,
    ) -> Result<Pin<Box<Self>>, AllocError> {
        assert!((flag == RT_IPC_FLAG_FIFO as u8) || (flag == RT_IPC_FLAG_PRIO as u8));

        rt_debug_not_in_interrupt!();

        let msg_align_size = align_up_size(msg_size as usize, RT_ALIGN_SIZE as usize);
        let msg_pool =
            // SAFETY: msg_align_size is a multiple of RT_ALIGN_SIZE and msg_pool is null checked
            unsafe { rt_malloc((msg_align_size as usize + mem::size_of::<RtMessage>())
                * max_msgs as usize) as *mut u8 } ;

        if msg_pool.is_null() {
            return Err(AllocError);
        }

        let mut msg_queue_free = null_mut();
        for temp in 0..max_msgs as usize {
            // SAFETY: msg_pool is null checked
            let head = unsafe {
                msg_pool.offset(
                    (temp * (msg_align_size as usize + mem::size_of::<RtMessage>())) as isize,
                ) as *mut RtMessage
            };

            unsafe {
                (*head).next = msg_queue_free as *mut RtMessage;
            }
            msg_queue_free = head as *mut u8;
        }

        Box::pin_init(pin_init!(Self {
            parent<-IPCObject::new(ObjectClassType::ObjectClassMessageQueue as u8, name, flag),
            msg_pool: msg_pool,
            msg_size: msg_size,
            max_msgs: max_msgs,
            entry: 0,
            msg_queue_head: null_mut(),
            msg_queue_tail: null_mut(),
            msg_queue_free: msg_queue_free,
            suspend_sender_thread<-ListHead::new()
        }))
    }
    #[inline]
    pub fn init(
        &mut self,
        name: *const i8,
        msg_pool: *mut u8,
        msg_size: usize,
        pool_size: usize,
        flag: u8,
    ) -> i32 {
        assert!((flag == RT_IPC_FLAG_FIFO as u8) || (flag == RT_IPC_FLAG_PRIO as u8));

        self.parent
            .init(ObjectClassType::ObjectClassMessageQueue as u8, name, flag);

        self.msg_pool = msg_pool as *mut u8;

        let msg_align_size = align_up_size(msg_size as usize, RT_ALIGN_SIZE as usize);
        self.msg_size = msg_size as u16;

        self.max_msgs =
            (pool_size as usize / (msg_align_size + mem::size_of::<RtMessage>())) as u16;

        if self.max_msgs == 0 {
            return -(RT_EINVAL as i32);
        }

        self.msg_queue_head = null_mut();
        self.msg_queue_tail = null_mut();

        self.msg_queue_free = null_mut();

        // SAFETY: ensure ptr not exceed the pool size
        unsafe {
            for temp in 0..self.max_msgs as usize {
                let head = (self.msg_pool as *mut u8).offset(
                    (temp * (msg_align_size as usize + mem::size_of::<RtMessage>())) as isize,
                ) as *mut RtMessage;
                (*head).next = self.msg_queue_free as *mut RtMessage;
                self.msg_queue_free = head as *mut u8;
            }
        }
        self.entry = 0;

        unsafe {
            let _ = ListHead::new().__pinned_init(&mut self.suspend_sender_thread as *mut ListHead);
        }

        RT_EOK as i32
    }

    pub fn init_new_storage(
        &mut self,
        name: *const i8,
        msg_size: u16,
        max_msgs: u16,
        flag: u8,
    ) -> i32 {
        self.parent
            .init(ObjectClassType::ObjectClassMessageQueue as u8, name, flag);

        self.msg_size = msg_size;
        self.max_msgs = max_msgs;
        self.entry = 0;
        self.msg_queue_head = null_mut();
        self.msg_queue_tail = null_mut();

        let msg_align_size = align_up_size(msg_size as usize, RT_ALIGN_SIZE as usize);
        self.msg_pool =
            // SAFETY: msg_align_size is a multiple of RT_ALIGN_SIZE and msg_pool is null checked
            unsafe { rt_malloc((msg_align_size as usize + mem::size_of::<RtMessage>())
                * max_msgs as usize) as *mut u8 };

        if self.msg_pool.is_null() {
            return -(RT_ENOMEM as i32);
        }

        self.msg_queue_free = null_mut();
        for temp in 0..max_msgs as usize {
            // SAFETY: msg_pool is null checked
            let head = unsafe {
                self.msg_pool.offset(
                    (temp * (msg_align_size as usize + mem::size_of::<RtMessage>())) as isize,
                ) as *mut RtMessage
            };

            unsafe {
                (*head).next = self.msg_queue_free as *mut RtMessage;
            }
            self.msg_queue_free = head as *mut u8;
        }

        unsafe {
            let _ = ListHead::new().__pinned_init(&mut self.suspend_sender_thread as *mut ListHead);
        }

        RT_EOK as i32
    }

    #[inline]
    pub fn detach(&mut self) {
        assert_eq!(
            self.type_name(),
            ObjectClassType::ObjectClassMessageQueue as u8
        );
        assert!(self.is_static_kobject());

        self.parent.wake_all();
        IPCObject::resume_all_threads(&mut self.suspend_sender_thread);
        self.parent.parent.detach();
    }

    #[inline]
    pub fn new_raw(
        name: *const i8,
        msg_size: usize,
        max_msgs: usize,
        flag: u8,
    ) -> *mut RtMessageQueue {
        let message_queue = RtMessageQueue::new(
            char_ptr_to_array(name),
            msg_size as u16,
            max_msgs as u16,
            flag,
        );
        match message_queue {
            Ok(mq) => unsafe { Box::leak(Pin::into_inner_unchecked(mq)) },
            Err(_) => return null_mut(),
        }
    }

    #[inline]
    pub fn delete_raw(&mut self) {
        assert_eq!(
            self.type_name(),
            ObjectClassType::ObjectClassMessageQueue as u8
        );
        assert!(!self.is_static_kobject());

        rt_debug_not_in_interrupt!();

        self.parent.wake_all();
        IPCObject::resume_all_threads(&mut self.suspend_sender_thread);
        // SAFETY: null protection
        unsafe {
            if !self.msg_pool.is_null() {
                rt_free(self.msg_pool as *mut c_void);
            }
        }
        self.parent.parent.delete();
    }

    fn send_wait_internal(
        &mut self,
        buffer: *const u8,
        size: usize,
        prio: i32,
        timeout: i32,
        suspend_flag: u32,
    ) -> i32 {
        let mut timeout = timeout;

        assert_eq!(
            self.type_name(),
            ObjectClassType::ObjectClassMessageQueue as u8
        );

        assert!(size != 0);

        #[allow(unused_variables)]
        let scheduler = timeout != 0;
        rt_debug_scheduler_available!(scheduler);

        if size > self.msg_size as usize {
            return -(RT_ERROR as i32);
        }

        let mut tick_delta = 0;
        let thread_ptr = crate::current_thread_ptr!();

        if thread_ptr.is_null() {
            return -(RT_ERROR as i32);
        }

        // SAFETY: thread_ptr is null checked
        let thread = unsafe { &mut *thread_ptr };

        unsafe {
            rt_object_hook_call!(
                rt_object_put_hook,
                &mut self.parent.parent as *mut KObjectBase as *mut rt_object
            );
        }

        self.parent.lock();

        let mut msg = self.msg_queue_free as *mut RtMessage;

        if msg.is_null() && timeout == 0 {
            self.parent.unlock();
            return -(RT_EFULL as i32);
        }

        while {
            msg = self.msg_queue_free as *mut RtMessage;
            msg
        } == null_mut()
        {
            thread.error = -(RT_EINTR as i32);

            if timeout == 0 {
                self.parent.unlock();
                return -(RT_EFULL as i32);
            }

            let ret = IPCObject::suspend_thread(
                &mut self.suspend_sender_thread,
                thread_ptr,
                self.parent.flag,
                suspend_flag as u32,
            );

            if ret != RT_EOK as i32 {
                self.parent.unlock();
                return ret;
            }

            if timeout > 0 {
                tick_delta = rt_tick_get();
                thread.thread_timer.timer_control(
                    RT_TIMER_CTRL_SET_TIME as u32,
                    (&mut timeout) as *mut i32 as *mut c_void,
                );
                thread.thread_timer.start();
            }

            self.parent.unlock();

            Cpu::get_current_scheduler().do_task_schedule();

            if thread.error != RT_EOK as i32 {
                return thread.error;
            }

            self.parent.lock();

            if timeout > 0 {
                tick_delta = rt_tick_get() - tick_delta;
                timeout -= tick_delta as i32;
                if timeout < 0 {
                    timeout = 0;
                }
            }
        }

        // SAFETY: msg is null checked and buffer is legal
        unsafe {
            self.msg_queue_free = (*msg).next as *mut u8;

            self.parent.unlock();

            (*msg).next = null_mut();

            (*msg).length = size as isize;

            rt_memcpy(
                get_message_addr!(msg),
                buffer as *const core::ffi::c_void,
                size as usize,
            );
        }

        self.parent.lock();

        cfg_if::cfg_if! {
            if #[cfg(feature = "RT_USING_MESSAGEQUEUE_PRIORITY")] {
                // SAFETY: msg is null checked
                unsafe { (*msg).prio = prio } ;
                if self.msg_queue_head == null_mut() {
                    self.msg_queue_head = msg as *mut u8;
                }

                let mut node = self.msg_queue_head as *mut RtMessage;
                let mut prev_node: *mut RtMessage = null_mut();

                // SAFETY: node, msg, prev_node is null checked
                unsafe {
                    while !node.is_null() {
                        if (*node).prio < (*msg).prio {
                            if (prev_node == null_mut()) {
                                self.msg_queue_head = msg as *mut u8;
                            } else {
                                (*prev_node).next = msg;
                            }

                            (*msg).next = node;
                            break;
                        }

                        if (*node).next == null_mut() {
                            if node != msg {
                                (*node).next = msg;
                            }
                            self.msg_queue_tail = msg as *mut u8;
                            break;
                        }
                        prev_node = node;
                        node = (*node).next;
                    }
                }
            } else {
                if self.msg_queue_tail != null_mut() {
                    // SAFETY: msg_queue_tail is not null
                    unsafe {
                        (*(self.msg_queue_tail as *mut RtMessage)).next = msg;
                    }
                }

                self.msg_queue_tail = msg as *mut u8;

                if self.msg_queue_head.is_null() {
                    self.msg_queue_head = msg as *mut u8;
                }
            }
        }

        if self.entry < RT_MQ_ENTRY_MAX as u16 {
            // increase message entry
            self.entry += 1;
        } else {
            self.parent.unlock();
            return -(RT_EFULL as i32);
        }

        if self.parent.has_waiting() {
            self.parent.wake_one();

            self.parent.unlock();

            Cpu::get_current_scheduler().do_task_schedule();

            return RT_EOK as i32;
        }

        self.parent.unlock();

        RT_EOK as i32
    }

    pub fn send(&mut self, buffer: *const u8, size: usize) -> i32 {
        self.send_wait(buffer, size, 0)
    }

    fn send_interruptible(&mut self, buffer: *const u8, size: usize) -> i32 {
        self.send_wait_interruptible(buffer, size, 0)
    }

    fn send_killable(&mut self, buffer: *const u8, size: usize) -> i32 {
        self.send_wait_killable(buffer, size, 0)
    }

    pub fn send_wait(&mut self, buffer: *const u8, size: usize, timeout: i32) -> i32 {
        self.send_wait_internal(buffer, size, 0, timeout, RT_UNINTERRUPTIBLE as u32)
    }

    pub fn send_wait_interruptible(&mut self, buffer: *const u8, size: usize, timeout: i32) -> i32 {
        self.send_wait_internal(buffer, size, 0, timeout, RT_INTERRUPTIBLE as u32)
    }

    pub fn send_wait_killable(&mut self, buffer: *const u8, size: usize, timeout: i32) -> i32 {
        self.send_wait_internal(buffer, size, 0, timeout, RT_KILLABLE as u32)
    }

    pub fn urgent(&mut self, buffer: *const u8, size: usize) -> rt_err_t {
        assert_eq!(
            self.type_name(),
            ObjectClassType::ObjectClassMessageQueue as u8
        );
        assert!(!buffer.is_null());
        assert!(size != 0);

        if size > self.msg_size as usize {
            return -(RT_ERROR as i32);
        }

        unsafe {
            rt_object_hook_call!(
                rt_object_put_hook,
                &mut self.parent.parent as *mut KObjectBase as *mut rt_object
            );
        }

        self.parent.lock();

        let msg = self.msg_queue_free as *mut RtMessage;

        if msg.is_null() {
            self.parent.unlock();
            return -(RT_EFULL as i32);
        }

        // SAFETY: msg is null checked and buffer is valid
        unsafe {
            self.msg_queue_free = (*msg).next as *mut u8;

            self.parent.unlock();

            (*msg).length = size as isize;

            rt_memcpy(
                get_message_addr!(msg),
                buffer as *const core::ffi::c_void,
                size as usize,
            );

            self.parent.lock();

            (*msg).next = self.msg_queue_head as *mut RtMessage;
        }
        self.msg_queue_head = msg as *mut u8;

        if self.msg_queue_tail.is_null() {
            self.msg_queue_tail = msg as *mut u8;
        }

        if self.entry < RT_MQ_ENTRY_MAX as u16 {
            self.entry += 1;
        } else {
            self.parent.unlock();
            return -(RT_EFULL as i32);
        }

        if !self.parent.has_waiting() {
            self.parent.wake_one();

            self.parent.unlock();

            Cpu::get_current_scheduler().do_task_schedule();

            return RT_EOK as i32;
        }

        self.parent.unlock();

        RT_EOK as i32
    }

    fn receive_internal(
        &mut self,
        buffer: *mut u8,
        size: usize,
        prio: *mut i32,
        timeout: i32,
        suspend_flag: u32,
    ) -> i32 {
        let mut timeout = timeout;

        assert_eq!(
            self.type_name(),
            ObjectClassType::ObjectClassMessageQueue as u8
        );
        assert!(!buffer.is_null());
        assert!(size != 0);

        #[allow(unused_variables)]
        let scheduler = timeout != 0;
        rt_debug_scheduler_available!(scheduler);

        let thread_ptr = crate::current_thread_ptr!();

        if thread_ptr.is_null() {
            return -(RT_ERROR as i32);
        }

        // SAFETY: thread_ptr is null checked
        let thread = unsafe { &mut *thread_ptr };

        unsafe {
            rt_object_hook_call!(
                rt_object_trytake_hook,
                &mut self.parent.parent as *mut KObjectBase as *mut rt_object
            );
        }

        self.parent.lock();

        if self.entry == 0 && timeout == 0 {
            self.parent.unlock();
            return -(RT_ETIMEOUT as i32);
        }

        while self.entry == 0 {
            thread.error = -(RT_EINTR as i32);

            if timeout == 0 {
                self.parent.unlock();
                thread.error = -(RT_ETIMEOUT as i32);
                return -(RT_ETIMEOUT as i32);
            }

            let ret = self
                .parent
                .wait(thread_ptr, self.parent.flag, suspend_flag as u32);
            if ret != RT_EOK as i32 {
                self.parent.unlock();
                return ret;
            }

            let mut tick_delta: rt_uint32_t = 0;
            if timeout > 0 {
                tick_delta = rt_tick_get();

                thread.thread_timer.timer_control(
                    RT_TIMER_CTRL_SET_TIME as u32,
                    (&mut timeout) as *mut i32 as *mut c_void,
                );
                (*thread).thread_timer.start();
            }

            self.parent.unlock();
            Cpu::get_current_scheduler().do_task_schedule();

            if thread.error != RT_EOK as i32 {
                return thread.error;
            }

            self.parent.lock();

            if timeout > 0 {
                tick_delta = rt_tick_get() - tick_delta;
                timeout -= tick_delta as rt_int32_t;
                if timeout < 0 {
                    timeout = 0;
                }
            }
        }

        let msg = self.msg_queue_head as *mut RtMessage;

        // SAFETY: msg is null checked
        unsafe { self.msg_queue_head = (*msg).next as *mut u8 };

        if self.msg_queue_tail == msg as *mut u8 {
            self.msg_queue_tail = null_mut();
        }

        if self.entry > 0 {
            self.entry -= 1;
        }

        self.parent.unlock();

        // SAFETY: msg is null checked
        let mut len = unsafe { (*msg).length as usize };

        if len > size {
            len = size;
        }

        // SAFETY: msg is null checked and buffer is valid
        unsafe {
            rt_memcpy(
                buffer as *mut core::ffi::c_void,
                get_message_addr!(msg),
                len,
            )
        };

        cfg_if::cfg_if! {
            if #[cfg(feature = "RT_USING_MESSAGEQUEUE_PRIORITY")] {
                if !prio.is_null() {
                    //SAFETY: msg is null checked and prio is valid
                    unsafe { *prio = (*msg).prio };
                }
            }
        }

        self.parent.lock();

        // SAFETY: msg is null checked
        unsafe { (*msg).next = self.msg_queue_free as *mut RtMessage };

        self.msg_queue_free = msg as *mut u8;

        if !self.suspend_sender_thread.is_empty() {
            IPCObject::resume_thread(&mut self.suspend_sender_thread);

            self.parent.unlock();

            unsafe {
                rt_object_hook_call!(
                    rt_object_take_hook,
                    &mut self.parent.parent as *mut KObjectBase as *mut rt_object
                );
            }

            Cpu::get_current_scheduler().do_task_schedule();

            return len as i32;
        }

        self.parent.unlock();

        unsafe {
            rt_object_hook_call!(
                rt_object_take_hook,
                &mut self.parent.parent as *mut KObjectBase as *mut rt_object
            );
        }

        len as i32
    }

    pub fn receive(&mut self, buffer: *mut u8, size: usize, timeout: i32) -> i32 {
        self.receive_internal(buffer, size, null_mut(), timeout, RT_UNINTERRUPTIBLE as u32)
    }

    pub fn receive_interruptible(&mut self, buffer: *mut u8, size: usize, timeout: i32) -> i32 {
        self.receive_internal(buffer, size, null_mut(), timeout, RT_INTERRUPTIBLE as u32)
    }

    pub fn receive_killable(&mut self, buffer: *mut u8, size: usize, timeout: i32) -> i32 {
        self.receive_internal(buffer, size, null_mut(), timeout, RT_KILLABLE as u32)
    }

    pub fn control(&mut self, cmd: i32, _arg: *mut u8) -> i32 {
        assert_eq!(
            self.type_name(),
            ObjectClassType::ObjectClassMessageQueue as u8
        );

        if cmd == RT_IPC_CMD_RESET as i32 {
            self.parent.lock();

            self.parent.wake_all();
            IPCObject::resume_all_threads(&mut self.suspend_sender_thread);

            // SAFETY: msg is valid
            unsafe {
                while !self.msg_queue_head.is_null() {
                    let msg = self.msg_queue_head as *mut RtMessage;

                    self.msg_queue_head = (*msg).next as *mut u8;
                    if self.msg_queue_tail == msg as *mut u8 {
                        self.msg_queue_tail = null_mut();
                    }

                    (*msg).next = self.msg_queue_free as *mut RtMessage;
                    self.msg_queue_free = msg as *mut u8;
                }
            }

            self.entry = 0;

            self.parent.unlock();

            Cpu::get_current_scheduler().do_task_schedule();

            return RT_EOK as i32;
        }

        -(RT_ERROR as i32)
    }

    #[cfg(feature = "RT_USING_MESSAGEQUEUE_PRIORITY")]
    fn send_wait_prio(
        &mut self,
        buffer: *const u8,
        size: usize,
        prio: i32,
        timeout: i32,
        suspend_flag: u32,
    ) -> rt_err_t {
        self.send_wait_internal(buffer, size, prio, timeout, suspend_flag)
    }

    #[cfg(feature = "RT_USING_MESSAGEQUEUE_PRIORITY")]
    fn receive_prio(
        &mut self,
        buffer: *mut u8,
        size: usize,
        prio: *mut i32,
        timeout: i32,
        suspend_flag: u32,
    ) -> rt_ssize_t {
        self.receive_internal(buffer, size, prio, timeout, suspend_flag)
    }
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_init(
    mq: *mut RtMessageQueue,
    name: *const core::ffi::c_char,
    msgpool: *mut core::ffi::c_void,
    msg_size: rt_size_t,
    pool_size: rt_size_t,
    flag: rt_uint8_t,
) -> rt_err_t {
    assert!(!mq.is_null());
    (*mq).init(
        name,
        msgpool as *mut u8,
        msg_size as usize,
        pool_size as usize,
        flag,
    )
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_detach(mq: *mut RtMessageQueue) -> rt_err_t {
    assert!(!mq.is_null());
    (*mq).detach();
    RT_EOK as rt_err_t
}

#[cfg(all(feature = "RT_USING_MESSAGEQUEUE", feature = "RT_USING_HEAP"))]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_create(
    name: *const core::ffi::c_char,
    msg_size: rt_size_t,
    max_msgs: rt_size_t,
    flag: rt_uint8_t,
) -> *mut RtMessageQueue {
    RtMessageQueue::new_raw(name, msg_size as usize, max_msgs as usize, flag)
}

#[cfg(all(feature = "RT_USING_MESSAGEQUEUE", feature = "RT_USING_HEAP"))]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_delete(mq: *mut RtMessageQueue) -> rt_err_t {
    assert!(mq != null_mut());

    (*mq).delete_raw();

    RT_EOK as rt_err_t
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_send(
    mq: *mut RtMessageQueue,
    buffer: *const core::ffi::c_void,
    size: rt_size_t,
) -> rt_err_t {
    assert!(!mq.is_null());
    (*mq).send(buffer as *const u8, size as usize)
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_send_interruptible(
    mq: *mut RtMessageQueue,
    buffer: *const core::ffi::c_void,
    size: rt_size_t,
) -> rt_err_t {
    assert!(!mq.is_null());
    (*mq).send_interruptible(buffer as *const u8, size as usize)
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_send_killable(
    mq: *mut RtMessageQueue,
    buffer: *const core::ffi::c_void,
    size: rt_size_t,
) -> rt_err_t {
    assert!(!mq.is_null());
    (*mq).send_killable(buffer as *const u8, size as usize)
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_send_wait(
    mq: *mut RtMessageQueue,
    buffer: *const core::ffi::c_void,
    size: rt_size_t,
    timeout: rt_int32_t,
) -> rt_err_t {
    assert!(!mq.is_null());
    (*mq).send_wait(buffer as *const u8, size as usize, timeout)
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_send_wait_interruptible(
    mq: *mut RtMessageQueue,
    buffer: *const core::ffi::c_void,
    size: rt_size_t,
    timeout: rt_int32_t,
) -> rt_err_t {
    assert!(!mq.is_null());
    (*mq).send_wait_interruptible(buffer as *const u8, size as usize, timeout)
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_send_wait_killable(
    mq: *mut RtMessageQueue,
    buffer: *const core::ffi::c_void,
    size: rt_size_t,
    timeout: rt_int32_t,
) -> rt_err_t {
    assert!(!mq.is_null());
    (*mq).send_wait_killable(buffer as *const u8, size as usize, timeout)
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_urgent(
    mq: *mut RtMessageQueue,
    buffer: *const core::ffi::c_void,
    size: rt_size_t,
) -> rt_err_t {
    assert!(!mq.is_null());
    (*mq).urgent(buffer as *const u8, size as usize)
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_recv(
    mq: *mut RtMessageQueue,
    buffer: *mut core::ffi::c_void,
    size: rt_size_t,
    timeout: rt_int32_t,
) -> rt_ssize_t {
    assert!(!mq.is_null());
    (*mq).receive(buffer as *mut u8, size as usize, timeout)
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_recv_interruptible(
    mq: *mut RtMessageQueue,
    buffer: *mut core::ffi::c_void,
    size: rt_size_t,
    timeout: rt_int32_t,
) -> rt_ssize_t {
    assert!(!mq.is_null());
    (*mq).receive_interruptible(buffer as *mut u8, size as usize, timeout)
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_recv_killable(
    mq: *mut RtMessageQueue,
    buffer: *mut core::ffi::c_void,
    size: rt_size_t,
    timeout: rt_int32_t,
) -> rt_ssize_t {
    assert!(!mq.is_null());
    (*mq).receive_killable(buffer as *mut u8, size as usize, timeout)
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_control(
    mq: *mut RtMessageQueue,
    cmd: core::ffi::c_int,
    _arg: *mut core::ffi::c_void,
) -> rt_err_t {
    assert!(!mq.is_null());
    (*mq).control(cmd, _arg as *mut u8)
}

#[cfg(all(
    feature = "RT_USING_MESSAGEQUEUE",
    feature = "RT_USING_MESSAGEQUEUE_PRIORITY"
))]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_send_wait_prio(
    mq: *mut RtMessageQueue,
    buffer: *const core::ffi::c_void,
    size: rt_size_t,
    prio: rt_int32_t,
    timeout: rt_int32_t,
    suspend_flag: core::ffi::c_int,
) -> rt_err_t {
    assert!(!mq.is_null());
    (*mq).send_wait_prio(
        buffer as *const u8,
        size as usize,
        prio,
        timeout,
        suspend_flag as u32,
    )
}

#[cfg(all(
    feature = "RT_USING_MESSAGEQUEUE",
    feature = "RT_USING_MESSAGEQUEUE_PRIORITY"
))]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_recv_prio(
    mq: *mut RtMessageQueue,
    buffer: *mut core::ffi::c_void,
    size: rt_size_t,
    prio: *mut rt_int32_t,
    timeout: rt_int32_t,
    suspend_flag: core::ffi::c_int,
) -> rt_ssize_t {
    assert!(!mq.is_null());
    (*mq).receive_prio(
        buffer as *mut u8,
        size as usize,
        prio,
        timeout,
        suspend_flag as u32,
    )
}

#[no_mangle]
#[allow(unused_unsafe)]
pub extern "C" fn rt_msgqueue_info() {
    let callback_forword = || {
        println!("msgqueue entry suspend thread");
        println!("-------- ----  --------------");
    };
    let callback = |node: &ListHead| unsafe {
        let msgqueue =
            &*(crate::list_head_entry!(node.as_ptr(), KObjectBase, list) as *const RtMessageQueue);
        let _ = crate::format_name!(msgqueue.parent.parent.name.as_ptr(), 8);
        print!(" {:04} ", msgqueue.entry);
        if msgqueue.parent.wait_list.is_empty() {
            println!(" {}", msgqueue.parent.wait_list.size());
        } else {
            print!(" {}:", msgqueue.parent.wait_list.size());
            let head = &msgqueue.parent.wait_list;
            list_head_for_each!(node, head, {
                let thread = crate::thread_list_node_entry!(node.as_ptr()) as *mut RtThread;
                let _ = crate::format_name!((*thread).parent.name.as_ptr(), 8);
            });
            print!("\n");
        }
    };
    let _ = KObjectBase::get_info(
        callback_forword,
        callback,
        ObjectClassType::ObjectClassMailBox as u8,
    );
}
