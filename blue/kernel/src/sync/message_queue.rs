use crate::{
    allocator::{align_up_size, rt_free, rt_malloc},
    clock::rt_tick_get,
    cpu::Cpu,
    error::Error,
    impl_kobject,
    klibc::rt_memcpy,
    linked_list::ListHead,
    list_head_for_each,
    object::{
        rt_object_allocate, rt_object_delete, rt_object_detach, rt_object_get_type, rt_object_init,
        rt_object_is_systemobject, ObjectClassType, *,
    },
    print, println,
    rt_bindings::*,
    sync::ipc_common::*,
    thread::RtThread,
    timer::{rt_timer_control, rt_timer_start, Timer},
};
#[allow(unused_imports)]
use core::{
    cell::UnsafeCell,
    ffi::{self, c_char, c_void},
    marker::PhantomPinned,
    mem,
    mem::MaybeUninit,
    ptr::null_mut,
};

use crate::sync::RawSpin;
use pinned_init::*;

#[macro_export]
macro_rules! get_message_addr {
    ($msg:expr) => {
        ($msg as *mut RtMessage).offset(1) as *mut _
    };
}

macro_rules! message_queue_priority {
    ($msg:expr, $mq:expr, $prio: expr) => {
        (*$msg).prio = $prio;
        if $mq.msg_queue_head == null_mut() {
            $mq.msg_queue_head = $msg as *mut c_void;
        }

        let node: *mut RtMessage = null_mut();
        let mut prev_node: *mut RtMessage = null_mut();

        while !node.is_null() {
            if (*node).prio < (*$msg).prio {
                if (prev_node == null_mut()) {
                    $mq.msg_queue_head = $msg as *mut c_void;
                } else {
                    (*prev_node).next = $msg;
                }

                (*$msg).next = node;
                break;
            }

            if (*node).next == null_mut() {
                if node != $msg {
                    (*node).next = $msg;
                }
                $mq.msg_queue_tail = $msg as *mut c_void;
                break;
            }
            prev_node = node;
        }
    };
}

#[allow(unused_macros)]
macro_rules! message_queue_non_prio {
    ($msg:expr, $mq:expr) => {
        if $mq.msg_queue_tail != null_mut() {
            (*($mq.msg_queue_tail as *mut RtMessage)).next = $msg
        }

        $mq.msg_queue_tail = $msg as *mut c_void;

        if $mq.msg_queue_head.is_null() {
            $mq.msg_queue_head = $msg as *mut c_void;
        }
    };
}

/// MessageQueue message structure
#[repr(C)]
#[pin_data]
pub struct RtMessage {
    #[pin]
    pub next: *mut RtMessage,
    pub length: ffi::c_long,
    pub prio: ffi::c_int,
}

/// MessageQueue raw structure
#[repr(C)]
#[pin_data]
pub struct RtMessageQueue {
    /// Inherit from IPCObject
    pub parent: IPCObject,
    /// Start address of message queue
    pub msg_pool: *mut ffi::c_void,
    /// Message size of each message
    pub msg_size: core::ffi::c_ushort,
    /// Max number of messages
    pub max_msgs: core::ffi::c_ushort,
    /// Index of messages in the queue
    pub entry: core::ffi::c_ushort,
    /// List head
    pub msg_queue_head: *mut ffi::c_void,
    /// List tail
    pub msg_queue_tail: *mut ffi::c_void,
    /// Pointer indicated the free node of queue
    pub msg_queue_free: *mut ffi::c_void,
    /// Sender thread suspended on this message queue
    #[pin]
    pub suspend_sender_thread: ListHead,
}

impl_kobject!(RtMessageQueue);

impl RtMessageQueue {
    #[inline]
    pub fn init(
        &mut self,
        name: *const i8,
        msg_pool: *mut core::ffi::c_void,
        msg_size: ffi::c_ulong,
        pool_size: ffi::c_ulong,
        flag: u8,
    ) -> ffi::c_long {
        assert!((flag == RT_IPC_FLAG_FIFO as u8) || (flag == RT_IPC_FLAG_PRIO as u8));

        self.parent
            .init(ObjectClassType::ObjectClassMessageQueue as u8, name, flag);

        self.msg_pool = msg_pool;

        let msg_align_size = align_up_size(msg_size as usize, RT_ALIGN_SIZE as usize);
        self.msg_size = msg_size as u16;

        self.max_msgs =
            (pool_size as usize / (msg_align_size + mem::size_of::<RtMessage>())) as u16;

        if self.max_msgs == 0 {
            return -(RT_EINVAL as ffi::c_long);
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
                self.msg_queue_free = head as *mut c_void;
            }
        }
        self.entry = 0;

        unsafe {
            let _ = ListHead::new().__pinned_init(&mut self.suspend_sender_thread as *mut ListHead);
        }

        RT_EOK as ffi::c_long
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
    pub fn new(
        name: *const i8,
        msg_size: ffi::c_ulong,
        max_msgs: ffi::c_ulong,
        flag: u8,
    ) -> *mut Self {
        assert!((flag == RT_IPC_FLAG_FIFO as u8) || (flag == RT_IPC_FLAG_PRIO as u8));

        rt_debug_not_in_interrupt!();

        // SAFETY: we have null ptr protection
        unsafe {
            let mq =
                IPCObject::new::<Self>(ObjectClassType::ObjectClassMessageQueue as u8, name, flag);
            if !mq.is_null() {
                let msg_align_size = align_up_size(msg_size as usize, RT_ALIGN_SIZE as usize);
                //unsafemonitor
                (*mq).msg_size = msg_size as u16;
                (*mq).max_msgs = max_msgs as u16;

                (*mq).msg_pool = rt_malloc(
                    (msg_align_size as usize + mem::size_of::<RtMessage>())
                        * (*mq).max_msgs as usize,
                );
                if (*mq).msg_pool == null_mut() {
                    (*mq).parent.parent.delete();
                    return null_mut();
                }

                (*mq).msg_queue_head = null_mut();
                (*mq).msg_queue_tail = null_mut();
                (*mq).msg_queue_free = null_mut();

                for temp in 0..(*mq).max_msgs as usize {
                    let head = ((*mq).msg_pool as *mut u8).offset(
                        (temp * (msg_align_size as usize + mem::size_of::<RtMessage>())) as isize,
                    ) as *mut RtMessage;

                    (*head).next = (*mq).msg_queue_free as *mut RtMessage;
                    (*mq).msg_queue_free = head as *mut c_void;
                }

                (*mq).entry = 0;
                unsafe {
                    let _ = ListHead::new()
                        .__pinned_init(&mut (*mq).suspend_sender_thread as *mut ListHead);
                }
            }
            mq
        }
    }

    #[inline]
    pub fn delete(&mut self) {
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
        buffer: *const core::ffi::c_void,
        size: ffi::c_ulong,
        prio: i32,
        timeout: i32,
        suspend_flag: u32,
    ) -> ffi::c_long {
        let mut timeout = timeout;

        assert_eq!(
            self.type_name(),
            ObjectClassType::ObjectClassMessageQueue as u8
        );

        assert!(size != 0);

        #[allow(unused_variables)]
        let scheduler = timeout != 0;
        rt_debug_scheduler_available!(scheduler);

        if size > self.msg_size as rt_size_t {
            return -(RT_ERROR as ffi::c_long);
        }

        let mut tick_delta = 0;
        let thread = unsafe { crate::current_thread!().unwrap().as_mut() };

        unsafe {
            rt_object_hook_call!(
                rt_object_put_hook,
                &mut self.parent.parent as *mut KObjectBase as *mut rt_object
            );

            self.parent.lock();

            let mut msg = self.msg_queue_free as *mut RtMessage;

            if msg.is_null() && timeout == 0 {
                self.parent.unlock();
                return -(RT_EFULL as ffi::c_long);
            }

            while {
                msg = self.msg_queue_free as *mut RtMessage;
                msg
            } == null_mut()
            {
                (*thread).error = -(RT_EINTR as ffi::c_long);

                if timeout == 0 {
                    self.parent.unlock();
                    return -(RT_EFULL as ffi::c_long);
                }

                let ret = IPCObject::suspend_thread(
                    &mut self.suspend_sender_thread,
                    thread,
                    self.parent.flag,
                    suspend_flag as u32,
                );

                if ret != RT_EOK as ffi::c_long {
                    self.parent.unlock();
                    return ret;
                }

                if timeout > 0 {
                    tick_delta = rt_tick_get();
                    (*thread).thread_timer.timer_control(
                        RT_TIMER_CTRL_SET_TIME as u32,
                        (&mut timeout) as *mut i32 as *mut c_void,
                    );
                    (*thread).thread_timer.start();
                }

                self.parent.unlock();

                Cpu::get_current_scheduler().do_task_schedule();

                if (*thread).error != RT_EOK as ffi::c_long {
                    return (*thread).error;
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

            self.msg_queue_free = (*msg).next as *mut c_void;

            self.parent.unlock();

            (*msg).next = null_mut();

            (*msg).length = size as ffi::c_long;

            rt_memcpy(get_message_addr!(msg), buffer, size as usize);

            self.parent.lock();

            #[cfg(feature = "RT_USING_MESSAGEQUEUE_PRIORITY")]
            {
                message_queue_priority!(msg, self, prio);
            }
            #[cfg(not(feature = "RT_USING_MESSAGEQUEUE_PRIORITY"))]
            {
                message_queue_non_prio!(msg, self);
            }

            if self.entry < RT_MQ_ENTRY_MAX as u16 {
                // increase message entry
                self.entry += 1;
            } else {
                self.parent.unlock();
                return -(RT_EFULL as ffi::c_long);
            }

            if self.parent.has_waiting() {
                self.parent.wake_one();

                self.parent.unlock();

                Cpu::get_current_scheduler().do_task_schedule();

                return RT_EOK as ffi::c_long;
            }

            self.parent.unlock();

            RT_EOK as ffi::c_long
        }
    }

    pub fn send(&mut self, buffer: *const core::ffi::c_void, size: ffi::c_ulong) -> ffi::c_long {
        self.send_wait(buffer, size, 0)
    }

    fn send_interruptible(
        &mut self,
        buffer: *const core::ffi::c_void,
        size: ffi::c_ulong,
    ) -> ffi::c_long {
        self.send_wait_interruptible(buffer, size, 0)
    }

    fn send_killable(
        &mut self,
        buffer: *const core::ffi::c_void,
        size: ffi::c_ulong,
    ) -> ffi::c_long {
        self.send_wait_killable(buffer, size, 0)
    }

    pub fn send_wait(
        &mut self,
        buffer: *const core::ffi::c_void,
        size: ffi::c_ulong,
        timeout: i32,
    ) -> ffi::c_long {
        self.send_wait_internal(buffer, size, 0, timeout, RT_UNINTERRUPTIBLE as u32)
    }

    pub fn send_wait_interruptible(
        &mut self,
        buffer: *const core::ffi::c_void,
        size: ffi::c_ulong,
        timeout: i32,
    ) -> ffi::c_long {
        self.send_wait_internal(buffer, size, 0, timeout, RT_INTERRUPTIBLE as u32)
    }

    pub fn send_wait_killable(
        &mut self,
        buffer: *const core::ffi::c_void,
        size: ffi::c_ulong,
        timeout: i32,
    ) -> ffi::c_long {
        self.send_wait_internal(buffer, size, 0, timeout, RT_KILLABLE as u32)
    }

    pub fn urgent(&mut self, buffer: *const core::ffi::c_void, size: ffi::c_ulong) -> rt_err_t {
        assert_eq!(
            self.type_name(),
            ObjectClassType::ObjectClassMessageQueue as u8
        );
        assert!(!buffer.is_null());
        assert!(size != 0);

        if size > self.msg_size as ffi::c_ulong {
            return -(RT_ERROR as ffi::c_long);
        }

        // SAFETY: hook and msg are valid
        unsafe {
            rt_object_hook_call!(
                rt_object_put_hook,
                &mut self.parent.parent as *mut KObjectBase as *mut rt_object
            );

            self.parent.lock();

            let msg = self.msg_queue_free as *mut RtMessage;

            if msg.is_null() {
                self.parent.unlock();
                return -(RT_EFULL as ffi::c_long);
            }

            self.msg_queue_free = (*msg).next as *mut c_void;

            self.parent.unlock();

            (*msg).length = size as ffi::c_long;

            rt_memcpy(get_message_addr!(msg), buffer, size as usize);

            self.parent.lock();

            (*msg).next = self.msg_queue_head as *mut RtMessage;

            self.msg_queue_head = msg as *mut c_void;

            if self.msg_queue_tail.is_null() {
                self.msg_queue_tail = msg as *mut c_void;
            }

            if self.entry < RT_MQ_ENTRY_MAX as u16 {
                self.entry += 1;
            } else {
                self.parent.unlock();
                return -(RT_EFULL as ffi::c_long);
            }

            if !self.parent.has_waiting() {
                self.parent.wake_one();

                self.parent.unlock();

                Cpu::get_current_scheduler().do_task_schedule();

                return RT_EOK as ffi::c_long;
            }

            self.parent.unlock();

            RT_EOK as ffi::c_long
        }
    }

    fn receive_internal(
        &mut self,
        buffer: *mut core::ffi::c_void,
        size: ffi::c_ulong,
        prio: *mut i32,
        timeout: i32,
        suspend_flag: u32,
    ) -> ffi::c_long {
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

        let thread = unsafe { crate::current_thread!().unwrap().as_mut() };

        // SAFETY: ensure non-null and not outside the range of msg_pool
        unsafe {
            rt_object_hook_call!(
                rt_object_trytake_hook,
                &mut self.parent.parent as *mut KObjectBase as *mut rt_object
            );

            self.parent.lock();

            if self.entry == 0 && timeout == 0 {
                self.parent.unlock();
                return -(RT_ETIMEOUT as ffi::c_long);
            }

            while self.entry == 0 {
                (*thread).error = -(RT_EINTR as ffi::c_long);

                if timeout == 0 {
                    self.parent.unlock();
                    (*thread).error = -(RT_ETIMEOUT as ffi::c_long);
                    return -(RT_ETIMEOUT as ffi::c_long);
                }

                let ret = self
                    .parent
                    .wait(thread, self.parent.flag, suspend_flag as u32);
                if ret != RT_EOK as ffi::c_long {
                    self.parent.unlock();
                    return ret;
                }

                let mut tick_delta: rt_uint32_t = 0;
                if timeout > 0 {
                    tick_delta = rt_tick_get();

                    (*thread).thread_timer.timer_control(
                        RT_TIMER_CTRL_SET_TIME as u32,
                        (&mut timeout) as *mut i32 as *mut c_void,
                    );
                    (*thread).thread_timer.start();
                }

                self.parent.unlock();
                Cpu::get_current_scheduler().do_task_schedule();

                if (*thread).error != RT_EOK as ffi::c_long {
                    return (*thread).error;
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

            self.msg_queue_head = (*msg).next as *mut c_void;

            if self.msg_queue_tail == msg as *mut c_void {
                self.msg_queue_tail = null_mut();
            }

            if self.entry > 0 {
                self.entry -= 1;
            }

            self.parent.unlock();

            let mut len = (*msg).length as rt_size_t;

            if len > size {
                len = size;
            }

            rt_memcpy(buffer, get_message_addr!(msg), len as usize);

            #[cfg(feature = "RT_USING_MESSAGEQUEUE_PRIORITY")]
            {
                if !prio.is_null() {
                    *prio = (*msg).prio;
                }
            }

            self.parent.lock();

            (*msg).next = self.msg_queue_free as *mut RtMessage;
            self.msg_queue_free = msg as *mut c_void;

            if !self.suspend_sender_thread.is_empty() {
                IPCObject::resume_thread(&mut self.suspend_sender_thread);

                self.parent.unlock();

                rt_object_hook_call!(
                    rt_object_take_hook,
                    &mut self.parent.parent as *mut KObjectBase as *mut rt_object
                );

                Cpu::get_current_scheduler().do_task_schedule();

                return len as ffi::c_long;
            }

            self.parent.unlock();

            rt_object_hook_call!(
                rt_object_take_hook,
                &mut self.parent.parent as *mut KObjectBase as *mut rt_object
            );

            len as ffi::c_long
        }
    }

    pub fn receive(
        &mut self,
        buffer: *mut core::ffi::c_void,
        size: ffi::c_ulong,
        timeout: i32,
    ) -> ffi::c_long {
        self.receive_internal(buffer, size, null_mut(), timeout, RT_UNINTERRUPTIBLE as u32)
    }

    pub fn receive_interruptible(
        &mut self,
        buffer: *mut core::ffi::c_void,
        size: ffi::c_ulong,
        timeout: i32,
    ) -> ffi::c_long {
        self.receive_internal(buffer, size, null_mut(), timeout, RT_INTERRUPTIBLE as u32)
    }

    pub fn receive_killable(
        &mut self,
        buffer: *mut core::ffi::c_void,
        size: ffi::c_ulong,
        timeout: i32,
    ) -> ffi::c_long {
        self.receive_internal(buffer, size, null_mut(), timeout, RT_KILLABLE as u32)
    }

    pub fn control(&mut self, cmd: i32, _arg: *mut core::ffi::c_void) -> ffi::c_long {
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

                    self.msg_queue_head = (*msg).next as *mut c_void;
                    if self.msg_queue_tail == msg as *mut c_void {
                        self.msg_queue_tail = null_mut();
                    }

                    (*msg).next = self.msg_queue_free as *mut RtMessage;
                    self.msg_queue_free = msg as *mut c_void;
                }
            }

            self.entry = 0;

            self.parent.unlock();

            Cpu::get_current_scheduler().do_task_schedule();

            return RT_EOK as ffi::c_long;
        }

        -(RT_ERROR as ffi::c_long)
    }

    #[cfg(feature = "RT_USING_MESSAGEQUEUE_PRIORITY")]
    fn send_wait_prio(
        &mut self,
        buffer: *const core::ffi::c_void,
        size: ffi::c_ulong,
        prio: i32,
        timeout: i32,
        suspend_flag: u32,
    ) -> rt_err_t {
        self.send_wait_internal(buffer, size, prio, timeout, suspend_flag);
    }

    #[cfg(feature = "RT_USING_MESSAGEQUEUE_PRIORITY")]
    fn receive_prio(
        &mut self,
        buffer: *mut core::ffi::c_void,
        size: ffi::c_ulong,
        prio: *mut i32,
        timeout: i32,
        suspend_flag: u32,
    ) -> rt_ssize_t {
        self.receive_internal(buffer, size, prio, timeout, suspend_flag);
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
    (*mq).init(name, msgpool, msg_size, pool_size, flag)
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
    RtMessageQueue::new(name, msg_size, max_msgs, flag)
}

#[cfg(all(feature = "RT_USING_MESSAGEQUEUE", feature = "RT_USING_HEAP"))]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_delete(mq: *mut RtMessageQueue) -> rt_err_t {
    assert!(mq != null_mut());

    (*mq).delete();

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
    (*mq).send(buffer, size)
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_send_interruptible(
    mq: *mut RtMessageQueue,
    buffer: *const core::ffi::c_void,
    size: rt_size_t,
) -> rt_err_t {
    assert!(!mq.is_null());
    (*mq).send_interruptible(buffer, size)
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_send_killable(
    mq: *mut RtMessageQueue,
    buffer: *const core::ffi::c_void,
    size: rt_size_t,
) -> rt_err_t {
    assert!(!mq.is_null());
    (*mq).send_killable(buffer, size)
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
    (*mq).send_wait(buffer, size, timeout)
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
    (*mq).send_wait_interruptible(buffer, size, timeout)
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
    (*mq).send_wait_killable(buffer, size, timeout)
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_urgent(
    mq: *mut RtMessageQueue,
    buffer: *const core::ffi::c_void,
    size: rt_size_t,
) -> rt_err_t {
    assert!(!mq.is_null());
    (*mq).urgent(buffer, size)
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
    (*mq).receive(buffer, size, timeout)
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
    (*mq).receive_interruptible(buffer, size, timeout)
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
    (*mq).receive_killable(buffer, size, timeout)
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_control(
    mq: *mut RtMessageQueue,
    cmd: core::ffi::c_int,
    _arg: *mut core::ffi::c_void,
) -> rt_err_t {
    assert!(!mq.is_null());
    (*mq).control(cmd, _arg)
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
    (*mq).send_wait_prio(buffer, size, prio, timeout, suspend_flag as u32)
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
    (*mq).rt_mq_recv_prio(buffer, size, prio, timeout, suspend_flag as u32)
}

#[pin_data]
pub struct MessageQueue {
    #[pin]
    mq_ptr: *mut RtMessageQueue,
    #[pin]
    _pin: PhantomPinned,
}

unsafe impl Send for MessageQueue {}
unsafe impl Sync for MessageQueue {}

impl MessageQueue {
    pub fn new(name: &str, msg_size: usize, max_msgs: usize) -> Result<Self, Error> {
        let result = unsafe {
            rt_mq_create(
                name.as_ptr() as *const c_char,
                msg_size as rt_size_t,
                max_msgs as rt_size_t,
                RT_IPC_FLAG_PRIO as u8,
            )
        };
        if result.is_null() {
            Err(Error::from_errno(RT_ERROR as i32))
        } else {
            Ok(Self {
                mq_ptr: result,
                _pin: PhantomPinned {},
            })
        }
    }

    pub fn delete(self) -> Result<(), Error> {
        let result = unsafe { rt_mq_delete(self.mq_ptr) };
        if result == RT_EOK as rt_err_t {
            Ok(())
        } else {
            Err(Error::from_errno(result))
        }
    }

    pub fn send(&self, msg: *const u8, size: usize) -> Result<(), Error> {
        self.send_wait(msg, size, 0)
    }

    pub fn send_interruptible(&self, msg: *const u8, size: usize) -> Result<(), Error> {
        let result = unsafe {
            rt_mq_send_interruptible(self.mq_ptr, msg as *const c_void, size as rt_size_t)
        };
        if result == RT_EOK as rt_err_t {
            Ok(())
        } else {
            Err(Error::from_errno(result))
        }
    }

    pub fn send_killable(&self, msg: *const u8, size: usize) -> Result<(), Error> {
        let result =
            unsafe { rt_mq_send_killable(self.mq_ptr, msg as *const c_void, size as rt_size_t) };
        if result == RT_EOK as rt_err_t {
            Ok(())
        } else {
            Err(Error::from_errno(result))
        }
    }

    pub fn send_wait(&self, msg: *const u8, size: usize, timeout: i32) -> Result<(), Error> {
        let result = unsafe {
            rt_mq_send_wait(
                self.mq_ptr,
                msg as *const c_void,
                size as rt_size_t,
                timeout as rt_int32_t,
            )
        };
        if result == RT_EOK as rt_err_t {
            Ok(())
        } else {
            Err(Error::from_errno(result))
        }
    }

    pub fn receive(&self, buffer: *mut u8, size: usize, timeout: i32) -> Result<usize, Error> {
        let result = unsafe {
            rt_mq_recv(
                self.mq_ptr,
                buffer as *mut core::ffi::c_void,
                size as rt_size_t,
                timeout as rt_int32_t,
            )
        };
        if result > 0 {
            Ok(result as usize)
        } else {
            Err(Error::from_errno(result))
        }
    }

    pub fn receive_interruptible(
        &self,
        buffer: *mut u8,
        size: usize,
        timeout: i32,
    ) -> Result<usize, Error> {
        let result = unsafe {
            rt_mq_recv_interruptible(
                self.mq_ptr,
                buffer as *mut core::ffi::c_void,
                size as rt_size_t,
                timeout as rt_int32_t,
            )
        };
        if result > 0 {
            Ok(result as usize)
        } else {
            Err(Error::from_errno(result))
        }
    }

    pub fn receive_killable(
        &self,
        buffer: *mut u8,
        size: usize,
        timeout: i32,
    ) -> Result<u32, Error> {
        let result = unsafe {
            rt_mq_recv_killable(
                self.mq_ptr,
                buffer as *mut core::ffi::c_void,
                size as rt_size_t,
                timeout as rt_int32_t,
            )
        };
        if result > 0 {
            Ok(result as u32)
        } else {
            Err(Error::from_errno(result))
        }
    }
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
