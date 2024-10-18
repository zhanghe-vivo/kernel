use crate::{
    allocator::{rt_free, rt_malloc},
    clock::rt_tick_get,
    error::Error,
    object::{
        rt_object_allocate, rt_object_delete, rt_object_detach, rt_object_get_type, rt_object_init,
        rt_object_is_systemobject, *,
    },
    rt_bindings::*,
    rt_debug_not_in_interrupt, rt_list_init,
    scheduler::rt_schedule,
    sync::ipc_common::*,
    thread::rt_thread_self,
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

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_init(
    mb: rt_mailbox_t,
    name: *const core::ffi::c_char,
    msgpool: *mut core::ffi::c_void,
    size: rt_size_t,
    flag: rt_uint8_t,
) -> rt_err_t {
    assert!(mb != null_mut());
    assert!((flag == RT_IPC_FLAG_FIFO as rt_uint8_t) || (flag == RT_IPC_FLAG_PRIO as rt_uint8_t));

    rt_object_init(
        &mut (*mb).parent.parent,
        rt_object_class_type_RT_Object_Class_MailBox as u32,
        name,
    );

    (*mb).parent.flag = flag;

    _rt_ipc_object_init(&mut (*mb).parent);

    (*mb).msg_pool = msgpool as *mut rt_ubase_t;
    (*mb).size = size as rt_uint16_t;
    (*mb).entry = 0;
    (*mb).in_offset = 0;
    (*mb).out_offset = 0;

    rt_list_init!(&mut (*mb).suspend_sender_thread);

    RT_EOK as rt_err_t
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_detach(mb: rt_mailbox_t) -> rt_err_t {
    assert!(mb != null_mut());
    assert!(
        rt_object_get_type(&mut (*mb).parent.parent)
            == rt_object_class_type_RT_Object_Class_MailBox as u8
    );
    assert!(rt_object_is_systemobject(&mut (*mb).parent.parent) == RT_TRUE as i32);

    _rt_ipc_list_resume_all(&mut ((*mb).parent.suspend_thread));
    _rt_ipc_list_resume_all(&mut ((*mb).suspend_sender_thread));

    rt_object_detach(&mut (*mb).parent.parent);

    return RT_EOK as rt_err_t;
}

#[cfg(all(feature = "RT_USING_MAILBOX", feature = "RT_USING_HEAP"))]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_create(
    name: *const core::ffi::c_char,
    size: rt_size_t,
    flag: rt_uint8_t,
) -> rt_mailbox_t {
    assert!((flag == RT_IPC_FLAG_FIFO as rt_uint8_t) || (flag == RT_IPC_FLAG_PRIO as rt_uint8_t));
    rt_debug_not_in_interrupt!();

    let mb = rt_object_allocate(rt_object_class_type_RT_Object_Class_MailBox as u32, name)
        as rt_mailbox_t;

    if mb == RT_NULL as rt_mailbox_t {
        return mb;
    }

    (*mb).parent.flag = flag;

    _rt_ipc_object_init(&mut (*mb).parent);

    (*mb).size = size as rt_uint16_t;
    let ptr = rt_malloc((*mb).size as usize * mem::size_of::<rt_ubase_t>()) as *mut rt_ubase_t;
    (*mb).msg_pool = ptr;
    if (*mb).msg_pool == null_mut() {
        rt_object_delete(&mut (*mb).parent.parent);
        return RT_NULL as rt_mailbox_t;
    }

    (*mb).entry = 0;
    (*mb).in_offset = 0;
    (*mb).out_offset = 0;

    rt_list_init!(&mut (*mb).suspend_sender_thread);

    mb
}

#[cfg(all(feature = "RT_USING_MAILBOX", feature = "RT_USING_HEAP"))]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_delete(mb: rt_mailbox_t) -> rt_err_t {
    assert!(mb != null_mut());
    assert!(
        rt_object_get_type(&mut (*mb).parent.parent)
            == rt_object_class_type_RT_Object_Class_MailBox as u8
    );
    assert!(rt_object_is_systemobject(&mut (*mb).parent.parent) == RT_FALSE as i32);

    rt_debug_not_in_interrupt!();

    _rt_ipc_list_resume_all(&mut (*mb).parent.suspend_thread);

    _rt_ipc_list_resume_all(&mut (*mb).suspend_sender_thread);

    rt_free((*mb).msg_pool as *mut c_void);

    rt_object_delete(&mut (*mb).parent.parent);

    RT_EOK as rt_err_t
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
unsafe extern "C" fn _rt_mb_send_wait(
    mb: rt_mailbox_t,
    value: rt_ubase_t,
    timeout: rt_int32_t,
    suspend_flag: i32,
) -> rt_err_t {
    assert!(mb != null_mut());
    assert!(
        rt_object_get_type(&mut (*mb).parent.parent)
            == rt_object_class_type_RT_Object_Class_MailBox as u8
    );

    let mut timeout = timeout;
    #[allow(unused_variables)]
    let scheduler = timeout != 0;
    rt_debug_scheduler_available!(scheduler);

    let mut tick_delta = 0;
    let thread = rt_thread_self();

    rt_object_hook_call!(rt_object_put_hook, &mut (*mb).parent.parent);

    let mut level = rt_hw_interrupt_disable();

    if (*mb).entry == (*mb).size && timeout == 0 {
        rt_hw_interrupt_enable(level);
        return -(RT_EFULL as rt_err_t);
    }

    while (*mb).entry == (*mb).size {
        (*thread).error = -(RT_EINTR as rt_err_t);

        if timeout == 0 {
            rt_hw_interrupt_enable(level);

            return -(RT_EFULL as rt_err_t);
        }

        let ret = _rt_ipc_list_suspend(
            &mut (*mb).suspend_sender_thread,
            thread as *mut rt_thread,
            (*mb).parent.flag,
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

    *(*mb).msg_pool.offset((*mb).in_offset as isize) = value;
    (*mb).in_offset += 1;
    if (*mb).in_offset >= (*mb).size {
        (*mb).in_offset = 0;
    }

    //unsafemonitor
    if (*mb).entry < RT_MB_ENTRY_MAX as rt_uint16_t {
        (*mb).entry += 1;
    } else {
        rt_hw_interrupt_enable(level);
        return -(RT_EFULL as rt_err_t);
    }

    if (*mb).parent.suspend_thread.is_empty() == false {
        _rt_ipc_list_resume(&mut (*mb).parent.suspend_thread);

        rt_hw_interrupt_enable(level);

        rt_schedule();

        return RT_EOK as rt_err_t;
    }

    rt_hw_interrupt_enable(level);

    return RT_EOK as rt_err_t;
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_send_wait(
    mb: rt_mailbox_t,
    value: rt_ubase_t,
    timeout: rt_int32_t,
) -> rt_err_t {
    _rt_mb_send_wait(mb, value, timeout, RT_UNINTERRUPTIBLE as i32)
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_send_wait_interruptible(
    mb: rt_mailbox_t,
    value: rt_ubase_t,
    timeout: rt_int32_t,
) -> rt_err_t {
    _rt_mb_send_wait(mb, value, timeout, RT_INTERRUPTIBLE as i32)
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_send_wait_killable(
    mb: rt_mailbox_t,
    value: rt_ubase_t,
    timeout: rt_int32_t,
) -> rt_err_t {
    _rt_mb_send_wait(mb, value, timeout, RT_KILLABLE as i32)
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_send(mb: rt_mailbox_t, value: rt_ubase_t) -> rt_err_t {
    rt_mb_send_wait(mb, value, 0)
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_send_interruptible(mb: rt_mailbox_t, value: rt_ubase_t) -> rt_err_t {
    rt_mb_send_wait_interruptible(mb, value, 0)
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_send_killable(mb: rt_mailbox_t, value: rt_ubase_t) -> rt_err_t {
    rt_mb_send_wait_killable(mb, value, 0)
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_urgent(mb: rt_mailbox_t, value: rt_ubase_t) -> rt_err_t {
    assert!(mb != null_mut());
    assert!(
        rt_object_get_type(&mut (*mb).parent.parent)
            == rt_object_class_type_RT_Object_Class_MailBox as u8
    );

    rt_object_hook_call!(rt_object_put_hook, &mut (*mb).parent.parent);

    let level = rt_hw_interrupt_disable();

    if (*mb).entry == (*mb).size {
        rt_hw_interrupt_enable(level);
        return -(RT_EFULL as rt_err_t);
    }

    if (*mb).out_offset > 0 {
        (*mb).out_offset -= 1;
    } else {
        (*mb).out_offset = (*mb).size - 1;
    }

    *(*mb).msg_pool.offset((*mb).out_offset as isize) = value;

    (*mb).entry += 1;

    if (*mb).parent.suspend_thread.is_empty() == false {
        _rt_ipc_list_resume(&mut (*mb).parent.suspend_thread);

        rt_hw_interrupt_enable(level);

        rt_schedule();

        return RT_EOK as rt_err_t;
    }

    rt_hw_interrupt_enable(level);

    RT_EOK as rt_err_t
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
unsafe extern "C" fn _rt_mb_recv(
    mb: rt_mailbox_t,
    value: *mut core::ffi::c_ulong,
    timeout: rt_int32_t,
    suspend_flag: i32,
) -> rt_err_t {
    assert!(mb != null_mut());
    assert!(
        rt_object_get_type(&mut (*mb).parent.parent)
            == rt_object_class_type_RT_Object_Class_MailBox as u8
    );

    let mut timeout = timeout;
    #[allow(unused_variables)]
    let scheduler = timeout != 0;
    rt_debug_scheduler_available!(scheduler);

    let mut tick_delta = 0;
    let thread = rt_thread_self();

    rt_object_hook_call!(rt_object_trytake_hook, &mut (*mb).parent.parent);

    let mut level = rt_hw_interrupt_disable();

    if (*mb).entry == 0 && timeout == 0 {
        rt_hw_interrupt_enable(level);
        return -(RT_ETIMEOUT as rt_err_t);
    }

    while (*mb).entry == 0 {
        (*thread).error = -(RT_EINTR as rt_err_t);

        if timeout == 0 {
            rt_hw_interrupt_enable(level);
            (*thread).error = -(RT_ETIMEOUT as rt_err_t);

            return -(RT_ETIMEOUT as rt_err_t);
        }

        let ret = _rt_ipc_list_suspend(
            &mut (*mb).parent.suspend_thread,
            thread as *mut rt_thread,
            (*mb).parent.flag,
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

    //unsafemonitor
    *value = *(*mb).msg_pool.offset((*mb).out_offset as isize);

    (*mb).out_offset += 1;
    if (*mb).out_offset >= (*mb).size {
        (*mb).out_offset = 0;
    }

    if (*mb).entry > 0 {
        (*mb).entry -= 1;
    }

    if (*mb).suspend_sender_thread.is_empty() == false {
        _rt_ipc_list_resume(&mut (*mb).suspend_sender_thread);

        rt_hw_interrupt_enable(level);

        rt_object_hook_call!(rt_object_take_hook, &mut (*mb).parent.parent);

        rt_schedule();

        return RT_EOK as rt_err_t;
    }

    rt_hw_interrupt_enable(level);

    rt_object_hook_call!(rt_object_take_hook, &mut (*mb).parent.parent);

    RT_EOK as rt_err_t
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_recv(
    mb: rt_mailbox_t,
    value: *mut core::ffi::c_ulong,
    timeout: rt_int32_t,
) -> rt_err_t {
    return _rt_mb_recv(mb, value, timeout, RT_UNINTERRUPTIBLE as i32);
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_recv_interruptible(
    mb: rt_mailbox_t,
    value: *mut core::ffi::c_ulong,
    timeout: rt_int32_t,
) -> rt_err_t {
    return _rt_mb_recv(mb, value, timeout, RT_INTERRUPTIBLE as i32);
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_recv_killable(
    mb: rt_mailbox_t,
    value: *mut core::ffi::c_ulong,
    timeout: rt_int32_t,
) -> rt_err_t {
    return _rt_mb_recv(mb, value, timeout, RT_KILLABLE as i32);
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_control(
    mb: rt_mailbox_t,
    cmd: core::ffi::c_int,
    _arg: *mut core::ffi::c_void,
) -> rt_err_t {
    assert!(mb != null_mut());
    assert!(
        rt_object_get_type(&mut (*mb).parent.parent)
            == rt_object_class_type_RT_Object_Class_MailBox as u8
    );

    if cmd == RT_IPC_CMD_RESET as i32 {
        let level = rt_hw_interrupt_disable();

        _rt_ipc_list_resume_all(&mut (*mb).parent.suspend_thread);
        _rt_ipc_list_resume_all(&mut (*mb).suspend_sender_thread);

        (*mb).entry = 0;
        (*mb).in_offset = 0;
        (*mb).out_offset = 0;

        rt_hw_interrupt_enable(level);

        rt_schedule();

        return RT_EOK as rt_err_t;
    }

    RT_EOK as rt_err_t
}

#[pin_data]
pub struct MailBox {
    #[pin]
    mb_ptr: rt_mailbox_t,
    #[pin]
    _pin: PhantomPinned,
}

unsafe impl Send for MailBox {}
unsafe impl Sync for MailBox {}

impl MailBox {
    pub fn new(name: &str, size: usize) -> Result<Self, Error> {
        let result = unsafe {
            rt_mb_create(
                name.as_ptr() as *const c_char,
                size as rt_size_t,
                RT_IPC_FLAG_PRIO as u8,
            )
        };
        if result == RT_NULL as rt_mailbox_t {
            Err(Error::from_errno(RT_ERROR as i32))
        } else {
            Ok(Self {
                mb_ptr: result,
                _pin: PhantomPinned {},
            })
        }
    }

    pub fn delete(self) -> Result<(), Error> {
        let result = unsafe { rt_mb_delete(self.mb_ptr) };
        if result == RT_EOK as rt_err_t {
            Ok(())
        } else {
            Err(Error::from_errno(result))
        }
    }

    pub fn send(&self, set: u32) -> Result<(), Error> {
        self.send_wait(set, 0)
    }

    pub fn send_wait(&self, set: u32, timeout: rt_int32_t) -> Result<(), Error> {
        let result = unsafe { rt_mb_send_wait(self.mb_ptr, set, timeout) };
        if result == RT_EOK as rt_err_t {
            Ok(())
        } else {
            Err(Error::from_errno(result))
        }
    }

    pub fn send_interruptible(&self, set: u32) -> Result<(), Error> {
        let result = unsafe { rt_mb_send_interruptible(self.mb_ptr, set as rt_ubase_t) };
        if result == RT_EOK as rt_err_t {
            Ok(())
        } else {
            Err(Error::from_errno(result))
        }
    }

    pub fn send_killable(&self, set: u32) -> Result<(), Error> {
        let result = unsafe { rt_mb_send_killable(self.mb_ptr, set as rt_ubase_t) };
        if result == RT_EOK as rt_err_t {
            Ok(())
        } else {
            Err(Error::from_errno(result))
        }
    }

    pub fn receive(&self, timeout: i32) -> Result<u32, Error> {
        let mut retmsg = 0 as core::ffi::c_ulong;
        let result = unsafe { rt_mb_recv(self.mb_ptr, &mut retmsg, timeout) };
        if result == RT_EOK as rt_err_t {
            Ok(retmsg as u32)
        } else {
            Err(Error::from_errno(result))
        }
    }

    pub fn receive_interruptible(&self, timeout: i32) -> Result<u32, Error> {
        let mut retmsg = 0 as core::ffi::c_ulong;
        let result = unsafe { rt_mb_recv_interruptible(self.mb_ptr, &mut retmsg, timeout) };
        if result == RT_EOK as rt_err_t {
            Ok(retmsg as u32)
        } else {
            Err(Error::from_errno(result))
        }
    }

    pub fn receive_killable(&self, timeout: i32) -> Result<u32, Error> {
        let mut retmsg = 0 as core::ffi::c_ulong;
        let result = unsafe { rt_mb_recv_killable(self.mb_ptr, &mut retmsg, timeout) };
        if result == RT_EOK as i32 {
            Ok(retmsg as u32)
        } else {
            Err(Error::from_errno(result))
        }
    }
}
