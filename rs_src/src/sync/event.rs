use crate::{
    error::Error,
    object::{
        rt_object_allocate, rt_object_delete, rt_object_detach, rt_object_get_type, rt_object_init,
        rt_object_is_systemobject, *,
    },
    rt_bindings::*,
    rt_debug_not_in_interrupt, rt_list_entry,
    scheduler::rt_schedule,
    sync::ipc_common::*,
    thread, timer,
};
use kernel::{rt_debug_scheduler_available, rt_object_hook_call};

use core::{
    cell::UnsafeCell,
    ffi::{c_char, c_void},
    marker::PhantomPinned,
    mem::MaybeUninit,
    ptr::null_mut,
};

use pinned_init::*;

#[cfg(feature = "RT_USING_EVENT")]
#[no_mangle]
pub unsafe extern "C" fn rt_event_init(
    event: rt_event_t,
    name: *const core::ffi::c_char,
    flag: rt_uint8_t,
) -> rt_err_t {
    assert!(event != null_mut());
    assert!((flag == RT_IPC_FLAG_FIFO as rt_uint8_t) || (flag == RT_IPC_FLAG_PRIO as rt_uint8_t));

    rt_object_init(
        &mut ((*event).parent.parent),
        rt_object_class_type_RT_Object_Class_Event as u32,
        name,
    );

    (*event).parent.parent.flag = flag;

    _rt_ipc_object_init(&mut ((*event).parent));

    (*event).set = 0;

    RT_EOK as rt_err_t
}

#[cfg(feature = "RT_USING_EVENT")]
#[no_mangle]
pub unsafe extern "C" fn rt_event_detach(event: rt_event_t) -> rt_err_t {
    assert!(event != null_mut());
    assert!(
        rt_object_get_type(&mut (*event).parent.parent)
            == rt_object_class_type_RT_Object_Class_Event as u8
    );
    assert!(rt_object_is_systemobject(&mut (*event).parent.parent) == RT_TRUE as i32);

    _rt_ipc_list_resume_all(&mut ((*event).parent.suspend_thread));

    rt_object_detach(&mut ((*event).parent.parent));

    RT_EOK as rt_err_t
}

#[cfg(all(feature = "RT_USING_EVENT", feature = "RT_USING_HEAP"))]
#[no_mangle]
pub unsafe extern "C" fn rt_event_create(
    name: *const core::ffi::c_char,
    flag: rt_uint8_t,
) -> rt_event_t {
    assert!((flag == RT_IPC_FLAG_FIFO as rt_uint8_t) || (flag == RT_IPC_FLAG_PRIO as rt_uint8_t));
    rt_debug_not_in_interrupt!();

    let event =
        rt_object_allocate(rt_object_class_type_RT_Object_Class_Event as u32, name) as rt_event_t;
    if event == RT_NULL as rt_event_t {
        return event;
    }

    (*event).parent.parent.flag = flag;

    _rt_ipc_object_init(&mut ((*event).parent));

    (*event).set = 0;

    event
}

#[cfg(all(feature = "RT_USING_EVENT", feature = "RT_USING_HEAP"))]
#[no_mangle]
pub unsafe extern "C" fn rt_event_delete(event: rt_event_t) -> rt_err_t {
    assert!(event != null_mut());
    assert!(
        rt_object_get_type(&mut (*event).parent.parent)
            == rt_object_class_type_RT_Object_Class_Event as u8
    );
    assert!(rt_object_is_systemobject(&mut (*event).parent.parent) == RT_FALSE as i32);

    rt_debug_not_in_interrupt!();

    _rt_ipc_list_resume_all(&mut ((*event).parent.suspend_thread));

    rt_object_delete(&mut ((*event).parent.parent));

    RT_EOK as rt_err_t
}

#[cfg(feature = "RT_USING_EVENT")]
#[no_mangle]
pub unsafe extern "C" fn rt_event_send(event: rt_event_t, set: rt_uint32_t) -> rt_err_t {
    let mut need_schedule = RT_FALSE;
    let mut need_clear_set = 0u32;

    assert!(event != null_mut());
    assert!(
        rt_object_get_type(&mut (*event).parent.parent)
            == rt_object_class_type_RT_Object_Class_Event as u8
    );

    if set == 0 {
        return -(RT_ERROR as rt_err_t);
    }

    let level = rt_hw_interrupt_disable();

    (*event).set |= set;

    rt_object_hook_call!(rt_object_put_hook, &mut ((*event).parent.parent));

    if (*event).parent.suspend_thread.is_empty() == false {
        let mut n = (*event).parent.suspend_thread.next;
        while n != &mut ((*event).parent.suspend_thread) {
            let thread = rt_list_entry!(n, rt_thread, tlist) as *mut rt_thread;
            let mut status = -(RT_ERROR as rt_err_t);
            if (*thread).event_info as u32 & RT_EVENT_FLAG_AND > 0u32 {
                if (*thread).event_set & (*event).set == (*thread).event_set {
                    status = RT_EOK as rt_err_t;
                }
            } else if (*thread).event_info as u32 & RT_EVENT_FLAG_OR > 0u32 {
                if (*thread).event_set & (*event).set > 0u32 {
                    (*thread).event_set = (*thread).event_set & (*event).set;
                    status = RT_EOK as rt_err_t;
                }
            } else {
                rt_hw_interrupt_enable(level);
                return -(RT_EINVAL as rt_err_t);
            }

            n = (*n).next;

            if status == RT_EOK as rt_err_t {
                if (*thread).event_info as u32 & RT_EVENT_FLAG_CLEAR > 0u32 {
                    need_clear_set |= (*thread).event_set;
                }

                thread::rt_thread_resume(thread as *mut thread::RtThread);
                (*thread).error = RT_EOK as rt_err_t;

                need_schedule = RT_TRUE;
            }
        }
        if need_clear_set > 0 {
            (*event).set &= !need_clear_set;
        }
    }
    rt_hw_interrupt_enable(level);

    if need_schedule == RT_TRUE {
        rt_schedule();
    }

    RT_EOK as rt_err_t
}

#[cfg(feature = "RT_USING_EVENT")]
#[no_mangle]
unsafe extern "C" fn _rt_event_recv(
    event: rt_event_t,
    set: rt_uint32_t,
    option: rt_uint8_t,
    timeout: rt_int32_t,
    recved: *mut rt_uint32_t,
    suspend_flag: i32,
) -> rt_err_t {
    assert!(event != null_mut());
    assert!(
        rt_object_get_type(&mut (*event).parent.parent)
            == rt_object_class_type_RT_Object_Class_Event as u8
    );

    rt_debug_scheduler_available!(RT_TRUE);

    if set == 0 {
        return -(RT_ERROR as rt_err_t);
    }

    let mut time_out = timeout;
    let mut status = -(RT_ERROR as rt_err_t);
    let thread = thread::rt_thread_self();
    (*thread).error = -(RT_EINTR as rt_err_t);

    rt_object_hook_call!(rt_object_trytake_hook, &mut ((*event).parent.parent));

    let mut level = rt_hw_interrupt_disable();

    if option as u32 & RT_EVENT_FLAG_AND > 0u32 {
        if ((*event).set & set) == set {
            status = RT_EOK as rt_err_t;
        }
    } else if option as u32 & RT_EVENT_FLAG_OR > 0u32 {
        if (*event).set & set > 0 {
            status = RT_EOK as rt_err_t;
        }
    } else {
        assert!(false);
    }

    if status == RT_EOK as rt_err_t {
        (*thread).error = RT_EOK as rt_err_t;

        if recved != null_mut() {
            *recved = (*event).set & set;
        }

        (*thread).event_set = (*event).set & set;
        (*thread).event_info = option;

        if option as u32 & RT_EVENT_FLAG_CLEAR > 0u32 {
            (*event).set &= !set;
        }
    } else if timeout == 0 {
        (*thread).error = -(RT_ETIMEOUT as rt_err_t);
        rt_hw_interrupt_enable(level);
        return -(RT_ETIMEOUT as rt_err_t);
    } else {
        (*thread).event_set = set;
        (*thread).event_info = option;

        let ret = _rt_ipc_list_suspend(
            &mut (*event).parent.suspend_thread,
            thread as *mut rt_thread,
            (*event).parent.parent.flag,
            suspend_flag,
        );
        if ret != RT_EOK as rt_err_t {
            rt_hw_interrupt_enable(level);
            return ret;
        }

        if timeout > 0 {
            timer::rt_timer_control(
                &mut (*thread).thread_timer as *const _ as *mut timer::Timer,
                RT_TIMER_CTRL_SET_TIME as i32,
                (&mut time_out) as *mut i32 as *mut c_void,
            );
            timer::rt_timer_start(&mut (*thread).thread_timer as *const _ as *mut timer::Timer);
        }

        rt_hw_interrupt_enable(level);

        rt_schedule();

        if (*thread).error != RT_EOK as rt_err_t {
            return (*thread).error;
        }

        level = rt_hw_interrupt_disable();

        if recved != null_mut() {
            *recved = (*thread).event_set;
        }
    }

    rt_hw_interrupt_enable(level);

    rt_object_hook_call!(rt_object_take_hook, &mut (*event).parent.parent);

    (*thread).error
}

#[cfg(feature = "RT_USING_EVENT")]
#[no_mangle]
pub unsafe extern "C" fn rt_event_recv(
    event: rt_event_t,
    set: rt_uint32_t,
    option: rt_uint8_t,
    timeout: rt_int32_t,
    recved: *mut rt_uint32_t,
) -> rt_err_t {
    _rt_event_recv(
        event,
        set,
        option,
        timeout,
        recved,
        RT_UNINTERRUPTIBLE as i32,
    )
}

#[cfg(feature = "RT_USING_EVENT")]
#[no_mangle]
pub unsafe extern "C" fn rt_event_recv_interruptible(
    event: rt_event_t,
    set: rt_uint32_t,
    option: rt_uint8_t,
    timeout: rt_int32_t,
    recved: *mut rt_uint32_t,
) -> rt_err_t {
    return _rt_event_recv(event, set, option, timeout, recved, RT_INTERRUPTIBLE as i32);
}

#[cfg(feature = "RT_USING_EVENT")]
#[no_mangle]
pub unsafe extern "C" fn rt_event_recv_killable(
    event: rt_event_t,
    set: rt_uint32_t,
    option: rt_uint8_t,
    timeout: rt_int32_t,
    recved: *mut rt_uint32_t,
) -> rt_err_t {
    _rt_event_recv(event, set, option, timeout, recved, RT_KILLABLE as i32)
}

#[cfg(feature = "RT_USING_EVENT")]
#[no_mangle]
pub unsafe extern "C" fn rt_event_control(
    event: rt_event_t,
    cmd: i32,
    _arg: *const c_void,
) -> rt_err_t {
    assert!(event != null_mut());
    assert!(
        rt_object_get_type(&mut (*event).parent.parent)
            == rt_object_class_type_RT_Object_Class_Event as u8
    );

    if cmd == RT_IPC_CMD_RESET as i32 {
        let level = rt_hw_interrupt_disable();

        _rt_ipc_list_resume_all(&mut (*event).parent.suspend_thread);

        (*event).set = 0;

        rt_hw_interrupt_enable(level);

        rt_schedule();

        return RT_EOK as rt_err_t;
    }

    -(RT_ERROR as rt_err_t)
}

#[pin_data]
pub struct Event {
    #[pin]
    event_ptr: rt_event_t,
    #[pin]
    _pin: PhantomPinned,
}

unsafe impl Send for Event {}
unsafe impl Sync for Event {}

impl Event {
    pub fn new(name: &str) -> Result<Self, Error> {
        let result =
            unsafe { rt_event_create(name.as_ptr() as *const c_char, RT_IPC_FLAG_PRIO as u8) };
        if result == RT_NULL as rt_event_t {
            Err(Error::from_errno(RT_ERROR as i32))
        } else {
            Ok(Self {
                event_ptr: result,
                _pin: PhantomPinned {},
            })
        }
    }

    pub fn delete(self) -> Result<(), Error> {
        let result = unsafe { rt_event_delete(self.event_ptr) };
        if result == RT_EOK as i32 {
            Ok(())
        } else {
            Err(Error::from_errno(result))
        }
    }

    pub fn send(&self, set: u32) -> Result<(), Error> {
        let result = unsafe { rt_event_send(self.event_ptr, set) };
        if result == RT_EOK as i32 {
            Ok(())
        } else {
            Err(Error::from_errno(result))
        }
    }

    pub fn receive(&self, set: u32, option: u32, timeout: i32) -> Result<u32, Error> {
        let mut retmsg = 0u32;
        let result =
            unsafe { rt_event_recv(self.event_ptr, set, option as u8, timeout, &mut retmsg) };
        if result == RT_EOK as i32 {
            Ok(retmsg)
        } else {
            Err(Error::from_errno(result))
        }
    }

    pub fn receive_interruptible(&self, set: u32, option: u32, timeout: i32) -> Result<u32, Error> {
        let mut retmsg = 0u32;
        let result = unsafe {
            rt_event_recv_interruptible(self.event_ptr, set, option as u8, timeout, &mut retmsg)
        };
        if result == RT_EOK as i32 {
            Ok(retmsg)
        } else {
            Err(Error::from_errno(result))
        }
    }

    pub fn receive_killable(&self, set: u32, option: u32, timeout: i32) -> Result<u32, Error> {
        let mut retmsg = 0u32;
        let result = unsafe {
            rt_event_recv_killable(self.event_ptr, set, option as u8, timeout, &mut retmsg)
        };
        if result == RT_EOK as i32 {
            Ok(retmsg)
        } else {
            Err(Error::from_errno(result))
        }
    }
}

#[pin_data]
pub struct EventStatic {
    #[pin]
    event_: UnsafeCell<MaybeUninit<rt_event_t>>,
    #[pin]
    pinned_: PhantomPinned,
}

unsafe impl Send for EventStatic {}
unsafe impl Sync for EventStatic {}

impl EventStatic {
    pub const fn new() -> Self {
        EventStatic {
            event_: UnsafeCell::new(MaybeUninit::uninit()),
            pinned_: PhantomPinned {},
        }
    }
    pub fn init(&'static self, name: &str) -> Result<(), Error> {
        let result = unsafe {
            rt_event_init(
                self.event_.get().cast(),
                name.as_ptr() as *const c_char,
                RT_IPC_FLAG_PRIO as u8,
            )
        };
        if result == RT_EOK as i32 {
            Ok(())
        } else {
            Err(Error::from_errno(result))
        }
    }

    pub fn detach(&'static self) -> Result<(), Error> {
        let result = unsafe { rt_event_detach(self.event_.get().cast()) };
        if result == RT_EOK as i32 {
            Ok(())
        } else {
            Err(Error::from_errno(result))
        }
    }

    pub fn get(&'static self) -> Event {
        Event {
            event_ptr: self.event_.get().cast(),
            _pin: PhantomPinned {},
        }
    }
}
