use crate::{
    allocator::{rt_free, rt_malloc},
    clock::rt_tick_get,
    error::Error,
    linked_list::ListHead,
    object::{
        rt_object_allocate, rt_object_delete, rt_object_detach, rt_object_get_type, rt_object_init,
        rt_object_is_systemobject, ObjectClassType, *,
    },
    print, println, rt_align,
    rt_bindings::*,
    rt_debug_not_in_interrupt, rt_get_message_addr, rt_list_init,
    scheduler::rt_schedule,
    sync::ipc_common::*,
    thread::{rt_thread_self, RtThread},
    timer::{rt_timer_control, rt_timer_start, Timer},
};
#[allow(unused_imports)]
use core::{
    cell::UnsafeCell,
    ffi::{c_char, c_void},
    marker::PhantomPinned,
    mem,
    mem::MaybeUninit,
    ptr::null_mut,
};
use kernel::{rt_debug_scheduler_available, rt_object_hook_call};

use pinned_init::*;

macro_rules! rt_message_queue_priority {
    ($msg:expr, $mq:expr, $prio: expr) => {
        (*$msg).prio = $prio;
        if (*$mq).msg_queue_head == null_mut() {
            (*$mq).msg_queue_head = $msg as *mut c_void;
        }

        let node: *mut rt_mq_message = null_mut();
        let mut prev_node: *mut rt_mq_message = null_mut();

        while !node.is_null() {
            if (*node).prio < (*$msg).prio {
                if (prev_node == null_mut()) {
                    (*$mq).msg_queue_head = $msg as *mut c_void;
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
                (*$mq).msg_queue_tail = $msg as *mut c_void;
                break;
            }
            prev_node = node;
        }
    };
}

#[allow(unused_macros)]
macro_rules! rt_message_queue_non_prio {
    ($msg:expr, $mq:expr) => {
        if (*$mq).msg_queue_tail != null_mut() {
            (*((*$mq).msg_queue_tail as *mut rt_mq_message)).next = $msg
        }

        (*$mq).msg_queue_tail = $msg as *mut c_void;

        if (*$mq).msg_queue_head.is_null() {
            (*$mq).msg_queue_head = $msg as *mut c_void;
        }
    };
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_init(
    mq: rt_mq_t,
    name: *const core::ffi::c_char,
    msgpool: *mut core::ffi::c_void,
    msg_size: rt_size_t,
    pool_size: rt_size_t,
    flag: rt_uint8_t,
) -> rt_err_t {
    assert!(mq != null_mut());
    assert!((flag == RT_IPC_FLAG_FIFO as rt_uint8_t) || (flag == RT_IPC_FLAG_PRIO as rt_uint8_t));

    rt_object_init(
        &mut (*mq).parent.parent,
        rt_object_class_type_RT_Object_Class_MessageQueue as u32,
        name,
    );

    (*mq).parent.flag = flag;

    _rt_ipc_object_init(&mut (*mq).parent);

    (*mq).msg_pool = msgpool;

    let msg_align_size = rt_align!(msg_size, RT_ALIGN_SIZE);
    (*mq).msg_size = msg_size as rt_uint16_t;
    (*mq).max_msgs = (pool_size / (msg_align_size + mem::size_of::<rt_mq_message>() as rt_size_t))
        as rt_uint16_t;

    if (*mq).max_msgs == 0 {
        return -(RT_EINVAL as rt_err_t);
    }

    (*mq).msg_queue_head = null_mut();
    (*mq).msg_queue_tail = null_mut();

    (*mq).msg_queue_free = null_mut();

    for temp in 0..(*mq).max_msgs as usize {
        let head = ((*mq).msg_pool as *mut rt_uint8_t)
            .offset((temp * (msg_align_size as usize + mem::size_of::<rt_mq_message>())) as isize)
            as *mut rt_mq_message;
        (*head).next = (*mq).msg_queue_free as *mut rt_mq_message;
        (*mq).msg_queue_free = head as *mut c_void;
    }

    (*mq).entry = 0;

    rt_list_init!(&mut (*mq).suspend_sender_thread);

    RT_EOK as rt_err_t
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_detach(mq: rt_mq_t) -> rt_err_t {
    assert!(mq != null_mut());
    assert!(
        rt_object_get_type(&mut (*mq).parent.parent)
            == rt_object_class_type_RT_Object_Class_MessageQueue as u8
    );
    assert!(rt_object_is_systemobject(&mut (*mq).parent.parent) == RT_TRUE as i32);

    _rt_ipc_list_resume_all(&mut (*mq).parent.suspend_thread);
    _rt_ipc_list_resume_all(&mut (*mq).suspend_sender_thread);

    rt_object_detach(&mut (*mq).parent.parent);

    RT_EOK as rt_err_t
}

#[cfg(all(feature = "RT_USING_MESSAGEQUEUE", feature = "RT_USING_HEAP"))]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_create(
    name: *const core::ffi::c_char,
    msg_size: rt_size_t,
    max_msgs: rt_size_t,
    flag: rt_uint8_t,
) -> rt_mq_t {
    assert!((flag == RT_IPC_FLAG_FIFO as rt_uint8_t) || (flag == RT_IPC_FLAG_PRIO as rt_uint8_t));

    rt_debug_not_in_interrupt!();

    let mq = rt_object_allocate(
        rt_object_class_type_RT_Object_Class_MessageQueue as u32,
        name,
    ) as *mut rt_messagequeue;
    if mq == null_mut() {
        return mq;
    }

    (*mq).parent.flag = flag;

    _rt_ipc_object_init(&mut (*mq).parent);

    let msg_align_size = rt_align!(msg_size, RT_ALIGN_SIZE);
    //unsafemonitor
    (*mq).msg_size = msg_size as rt_uint16_t;
    (*mq).max_msgs = max_msgs as rt_uint16_t;

    (*mq).msg_pool = rt_malloc(
        msg_align_size as usize + mem::size_of::<rt_mq_message>() * (*mq).max_msgs as usize,
    );
    if (*mq).msg_pool == null_mut() {
        rt_object_delete(&mut (*mq).parent.parent);
        return null_mut();
    }

    (*mq).msg_queue_head = null_mut();
    (*mq).msg_queue_tail = null_mut();

    (*mq).msg_queue_free = null_mut();
    for temp in 0..(*mq).max_msgs as usize {
        let head = ((*mq).msg_pool as *mut rt_uint8_t)
            .offset((temp * (msg_align_size as usize + mem::size_of::<rt_mq_message>())) as isize)
            as *mut rt_mq_message;

        (*head).next = (*mq).msg_queue_free as *mut rt_mq_message;
        (*mq).msg_queue_free = head as *mut c_void;
    }

    (*mq).entry = 0;

    rt_list_init!(&mut (*mq).suspend_sender_thread);

    return mq;
}

#[cfg(all(feature = "RT_USING_MESSAGEQUEUE", feature = "RT_USING_HEAP"))]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_delete(mq: rt_mq_t) -> rt_err_t {
    assert!(mq != null_mut());
    assert!(
        rt_object_get_type(&mut (*mq).parent.parent)
            == rt_object_class_type_RT_Object_Class_MessageQueue as u8
    );
    assert!(rt_object_is_systemobject(&mut (*mq).parent.parent) == RT_FALSE as i32);

    rt_debug_not_in_interrupt!();

    _rt_ipc_list_resume_all(&mut (*mq).parent.suspend_thread);
    _rt_ipc_list_resume_all(&mut (*mq).suspend_sender_thread);

    rt_free((*mq).msg_pool);

    rt_object_delete(&mut (*mq).parent.parent);

    RT_EOK as rt_err_t
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
unsafe extern "C" fn _rt_mq_send_wait(
    mq: rt_mq_t,
    buffer: *const core::ffi::c_void,
    size: rt_size_t,
    prio: rt_int32_t,
    timeout: rt_int32_t,
    suspend_flag: i32,
) -> rt_err_t {
    let mut timeout = timeout;

    assert!(mq != null_mut());
    assert!(
        rt_object_get_type(&mut (*mq).parent.parent)
            == rt_object_class_type_RT_Object_Class_MessageQueue as u8
    );
    assert!(buffer != null_mut());
    assert!(size != 0);

    #[allow(unused_variables)]
    let scheduler = timeout != 0;
    rt_debug_scheduler_available!(scheduler);

    if size > (*mq).msg_size as rt_size_t {
        return -(RT_ERROR as rt_err_t);
    }

    let mut tick_delta = 0;
    let thread = rt_thread_self();

    rt_object_hook_call!(rt_object_put_hook, &mut (*mq).parent.parent);

    let mut level = rt_hw_interrupt_disable();

    let mut msg = (*mq).msg_queue_free as *mut rt_mq_message;
    if msg.is_null() && timeout == 0 {
        rt_hw_interrupt_enable(level);
        return -(RT_EFULL as rt_err_t);
    }

    while {
        msg = (*mq).msg_queue_free as *mut rt_mq_message;
        msg
    } == null_mut()
    {
        (*thread).error = -(RT_EINTR as rt_err_t);

        if timeout == 0 {
            rt_hw_interrupt_enable(level);
            return -(RT_EFULL as rt_err_t);
        }

        let ret = _rt_ipc_list_suspend(
            &mut (*mq).suspend_sender_thread,
            thread as *mut rt_thread,
            (*mq).parent.flag,
            suspend_flag,
        );

        if ret != RT_EOK as rt_err_t {
            rt_hw_interrupt_enable(level);
            return ret;
        }

        if timeout > 0 {
            tick_delta = rt_tick_get();

            rt_timer_control(
                &mut (*thread).thread_timer as *const _ as *mut Timer,
                RT_TIMER_CTRL_SET_TIME as i32,
                (&mut timeout) as *mut i32 as *mut c_void,
            );
            rt_timer_start(&mut (*thread).thread_timer as *const _ as *mut Timer);
        }

        rt_hw_interrupt_enable(level);

        rt_schedule();

        if (*thread).error != RT_EOK as rt_err_t {
            return (*thread).error;
        }

        level = rt_hw_interrupt_disable();

        if timeout > 0 {
            tick_delta = rt_tick_get() - tick_delta;
            timeout -= tick_delta as rt_int32_t;
            if timeout < 0 {
                timeout = 0;
            }
        }
    }

    (*mq).msg_queue_free = (*msg).next as *mut c_void;

    rt_hw_interrupt_enable(level);

    (*msg).next = null_mut();

    (*msg).length = size as rt_ssize_t;

    _rt_memcpy(rt_get_message_addr!(msg), buffer, size as usize);

    rt_hw_interrupt_disable();

    #[cfg(feature = "RT_USING_MESSAGEQUEUE_PRIORITY")]
    {
        rt_message_queue_priority!(msg, mq, prio);
    }
    #[cfg(not(feature = "RT_USING_MESSAGEQUEUE_PRIORITY"))]
    {
        rt_message_queue_non_prio!(msg, mq);
    }
    RT_EOK as rt_err_t
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_send(
    mq: rt_mq_t,
    buffer: *const core::ffi::c_void,
    size: rt_size_t,
) -> rt_err_t {
    return rt_mq_send_wait(mq, buffer, size, 0);
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_send_interruptible(
    mq: rt_mq_t,
    buffer: *const core::ffi::c_void,
    size: rt_size_t,
) -> rt_err_t {
    rt_mq_send_wait_interruptible(mq, buffer, size, 0)
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_send_killable(
    mq: rt_mq_t,
    buffer: *const core::ffi::c_void,
    size: rt_size_t,
) -> rt_err_t {
    rt_mq_send_wait_killable(mq, buffer, size, 0)
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_send_wait(
    mq: rt_mq_t,
    buffer: *const core::ffi::c_void,
    size: rt_size_t,
    timeout: rt_int32_t,
) -> rt_err_t {
    _rt_mq_send_wait(mq, buffer, size, 0, timeout, RT_UNINTERRUPTIBLE as i32)
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_send_wait_interruptible(
    mq: rt_mq_t,
    buffer: *const core::ffi::c_void,
    size: rt_size_t,
    timeout: rt_int32_t,
) -> rt_err_t {
    _rt_mq_send_wait(mq, buffer, size, 0, timeout, RT_INTERRUPTIBLE as i32)
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_send_wait_killable(
    mq: rt_mq_t,
    buffer: *const core::ffi::c_void,
    size: rt_size_t,
    timeout: rt_int32_t,
) -> rt_err_t {
    _rt_mq_send_wait(mq, buffer, size, 0, timeout, RT_KILLABLE as i32)
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_urgent(
    mq: rt_mq_t,
    buffer: *const core::ffi::c_void,
    size: rt_size_t,
) -> rt_err_t {
    assert!(mq != null_mut());
    assert!(
        rt_object_get_type(&mut (*mq).parent.parent)
            == rt_object_class_type_RT_Object_Class_MessageQueue as u8
    );
    assert!(buffer != null_mut());
    assert!(size != 0);

    if size > (*mq).msg_size as rt_size_t {
        return -(RT_ERROR as rt_err_t);
    }

    rt_object_hook_call!(rt_object_put_hook, &mut (*mq).parent.parent);

    let mut level = rt_hw_interrupt_disable();

    let msg = (*mq).msg_queue_free as *mut rt_mq_message;
    if msg == null_mut() {
        rt_hw_interrupt_enable(level);
        return -(RT_EFULL as rt_err_t);
    }

    (*mq).msg_queue_free = (*msg).next as *mut c_void;

    rt_hw_interrupt_enable(level);

    (*msg).length = size as rt_ssize_t;

    _rt_memcpy(rt_get_message_addr!(msg), buffer, size as usize);

    level = rt_hw_interrupt_disable();

    (*msg).next = (*mq).msg_queue_head as *mut rt_mq_message;
    (*mq).msg_queue_head = msg as *mut c_void;

    if (*mq).msg_queue_tail.is_null() {
        (*mq).msg_queue_tail = msg as *mut c_void;
    }

    if (*mq).entry < RT_MQ_ENTRY_MAX as rt_uint16_t {
        (*mq).entry += 1;
    } else {
        rt_hw_interrupt_enable(level);
        return -(RT_EFULL as rt_err_t);
    }

    if (*mq).parent.suspend_thread.is_empty() == false {
        _rt_ipc_list_resume(&mut (*mq).parent.suspend_thread);

        rt_hw_interrupt_enable(level);

        rt_schedule();

        return RT_EOK as rt_err_t;
    }

    rt_hw_interrupt_enable(level);

    RT_EOK as rt_err_t
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
unsafe extern "C" fn _rt_mq_recv(
    mq: rt_mq_t,
    buffer: *mut core::ffi::c_void,
    size: rt_size_t,
    prio: *mut rt_int32_t,
    timeout: rt_int32_t,
    suspend_flag: i32,
) -> rt_ssize_t {
    let mut timeout = timeout;

    assert!(mq != null_mut());
    assert!(
        rt_object_get_type(&mut (*mq).parent.parent)
            == rt_object_class_type_RT_Object_Class_MessageQueue as u8
    );
    assert!(buffer != null_mut());
    assert!(size != 0);

    #[allow(unused_variables)]
    let scheduler = timeout != 0;
    rt_debug_scheduler_available!(scheduler);

    let thread = rt_thread_self();
    rt_object_hook_call!(rt_object_trytake_hook, &mut (*mq).parent.parent);

    let mut level = rt_hw_interrupt_disable();

    if (*mq).entry == 0 && timeout == 0 {
        rt_hw_interrupt_enable(level);
        return -(RT_ETIMEOUT as rt_err_t);
    }

    while (*mq).entry == 0 {
        (*thread).error = -(RT_EINTR as rt_err_t);

        if timeout == 0 {
            rt_hw_interrupt_enable(level);
            (*thread).error = -(RT_ETIMEOUT as rt_err_t);
            return -(RT_ETIMEOUT as rt_err_t);
        }

        let ret = _rt_ipc_list_suspend(
            &mut (*mq).parent.suspend_thread,
            thread as *mut rt_thread,
            (*mq).parent.flag,
            suspend_flag,
        );
        if ret != RT_EOK as rt_err_t {
            rt_hw_interrupt_enable(level);
            return ret;
        }

        let mut tick_delta: rt_uint32_t = 0;
        if timeout > 0 {
            tick_delta = rt_tick_get();

            rt_timer_control(
                &mut (*thread).thread_timer as *const _ as *mut Timer,
                RT_TIMER_CTRL_SET_TIME as i32,
                (&mut timeout) as *mut i32 as *mut c_void,
            );
            rt_timer_start(&mut (*thread).thread_timer as *const _ as *mut Timer);
        }

        rt_hw_interrupt_enable(level);
        rt_schedule();

        if (*thread).error != RT_EOK as rt_err_t {
            return (*thread).error;
        }

        level = rt_hw_interrupt_disable();

        if timeout > 0 {
            tick_delta = rt_tick_get() - tick_delta;
            timeout -= tick_delta as rt_int32_t;
            if timeout < 0 {
                timeout = 0;
            }
        }
    }

    let msg = (*mq).msg_queue_head as *mut rt_mq_message;

    (*mq).msg_queue_head = (*msg).next as *mut c_void;

    if (*mq).msg_queue_tail == msg as *mut c_void {
        (*mq).msg_queue_tail = null_mut();
    }

    if (*mq).entry > 0 {
        (*mq).entry -= 1;
    }

    rt_hw_interrupt_enable(level);

    let mut len = (*msg).length as rt_size_t;

    if len > size {
        len = size;
    }

    _rt_memcpy(buffer, rt_get_message_addr!(msg), len as usize);

    #[cfg(feature = "RT_USING_MESSAGEQUEUE_PRIORITY")]
    {
        if prio != null_mut() {
            *prio = (*msg).prio;
        }
    }

    level = rt_hw_interrupt_disable();

    (*msg).next = (*mq).msg_queue_free as *mut rt_mq_message;
    (*mq).msg_queue_free = msg as *mut c_void;

    if (*mq).suspend_sender_thread.is_empty() == false {
        _rt_ipc_list_resume(&mut (*mq).suspend_sender_thread);

        rt_hw_interrupt_enable(level);

        rt_object_hook_call!(rt_object_take_hook, &mut (*mq).parent.parent);

        rt_schedule();

        return len as rt_ssize_t;
    }

    rt_hw_interrupt_enable(level);

    rt_object_hook_call!(rt_object_take_hook, &mut (*mq).parent.parent);

    len as rt_ssize_t
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_recv(
    mq: rt_mq_t,
    buffer: *mut core::ffi::c_void,
    size: rt_size_t,
    timeout: rt_int32_t,
) -> rt_ssize_t {
    _rt_mq_recv(
        mq,
        buffer,
        size,
        null_mut(),
        timeout,
        RT_UNINTERRUPTIBLE as i32,
    )
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_recv_interruptible(
    mq: rt_mq_t,
    buffer: *mut core::ffi::c_void,
    size: rt_size_t,
    timeout: rt_int32_t,
) -> rt_ssize_t {
    _rt_mq_recv(
        mq,
        buffer,
        size,
        null_mut(),
        timeout,
        RT_INTERRUPTIBLE as i32,
    )
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_recv_killable(
    mq: rt_mq_t,
    buffer: *mut core::ffi::c_void,
    size: rt_size_t,
    timeout: rt_int32_t,
) -> rt_ssize_t {
    _rt_mq_recv(mq, buffer, size, null_mut(), timeout, RT_KILLABLE as i32)
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_control(
    mq: rt_mq_t,
    cmd: core::ffi::c_int,
    _arg: *mut core::ffi::c_void,
) -> rt_err_t {
    assert!(mq != null_mut());
    assert!(
        rt_object_get_type(&mut (*mq).parent.parent)
            == rt_object_class_type_RT_Object_Class_MessageQueue as u8
    );

    if cmd == RT_IPC_CMD_RESET as i32 {
        let level = rt_hw_interrupt_disable();

        _rt_ipc_list_resume_all(&mut (*mq).parent.suspend_thread);
        _rt_ipc_list_resume_all(&mut (*mq).suspend_sender_thread);

        while (*mq).msg_queue_head.is_null() == false {
            let msg = (*mq).msg_queue_head as *mut rt_mq_message;

            (*mq).msg_queue_head = (*msg).next as *mut c_void;
            if (*mq).msg_queue_tail == msg as *mut c_void {
                (*mq).msg_queue_tail = null_mut();
            }

            (*msg).next = (*mq).msg_queue_free as *mut rt_mq_message;
            (*mq).msg_queue_free = msg as *mut c_void;
        }

        (*mq).entry = 0;

        rt_hw_interrupt_enable(level);

        rt_schedule();

        return RT_EOK as rt_err_t;
    }

    -(RT_ERROR as rt_err_t)
}

#[cfg(all(
    feature = "RT_USING_MESSAGEQUEUE",
    feature = "RT_USING_MESSAGEQUEUE_PRIORITY"
))]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_send_wait_prio(
    mq: rt_mq_t,
    buffer: *const core::ffi::c_void,
    size: rt_size_t,
    prio: rt_int32_t,
    timeout: rt_int32_t,
    suspend_flag: core::ffi::c_int,
) -> rt_err_t {
    return _rt_mq_send_wait(mq, buffer, size, prio, timeout, suspend_flag);
}

#[cfg(all(
    feature = "RT_USING_MESSAGEQUEUE",
    feature = "RT_USING_MESSAGEQUEUE_PRIORITY"
))]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_recv_prio(
    mq: rt_mq_t,
    buffer: *mut core::ffi::c_void,
    size: rt_size_t,
    prio: *mut rt_int32_t,
    timeout: rt_int32_t,
    suspend_flag: core::ffi::c_int,
) -> rt_ssize_t {
    return _rt_mq_recv(mq, buffer, size, prio, timeout, suspend_flag);
}

#[pin_data]
pub struct MessageQueue {
    #[pin]
    mq_ptr: rt_mq_t,
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
            &*(crate::list_head_entry!(node.as_ptr(), KObjectBase, list) as *const rt_messagequeue);
        let _ = crate::format_name!(msgqueue.parent.parent.name.as_ptr(), 8);
        print!(" {:04} ", msgqueue.entry);
        if msgqueue.parent.suspend_thread.is_empty() {
            println!(" {}", msgqueue.parent.suspend_thread.len());
        } else {
            print!(" {}:", msgqueue.parent.suspend_thread.len());
            let head = &msgqueue.parent.suspend_thread;
            let mut list = head.next;
            loop {
                let thread_node = list;
                if thread_node == head as *const _ as *mut rt_list_node {
                    break;
                }
                let thread = &*crate::container_of!(thread_node, RtThread, tlist);
                let _ = crate::format_name!(thread.parent.name.as_ptr(), 8);
                list = (*list).next;
            }
            print!("\n");
        }
    };
    let _ = KObjectBase::get_info(
        callback_forword,
        callback,
        ObjectClassType::ObjectClassMailBox as u8,
    );
}
