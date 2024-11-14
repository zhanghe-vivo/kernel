use crate::{
    cpu::Cpu,
    error::Error,
    impl_kobject,
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
    timer,
};
use core::{
    ffi::{self, c_char, c_void},
    marker::PhantomPinned,
    ptr::null_mut,
};
use kernel::rt_bindings::{
    rt_debug_scheduler_available, rt_hw_interrupt_disable, rt_hw_interrupt_enable,
    rt_object_hook_call,
};

use crate::alloc::boxed::Box;
use core::cell::UnsafeCell;
use core::pin::Pin;
use kernel::{fmt, str::CString};

use crate::sync::RawSpin;
use pinned_init::*;

#[pin_data(PinnedDrop)]
pub struct KEvent {
    #[pin]
    raw: UnsafeCell<RtEvent>,
    #[pin]
    pin: PhantomPinned,
}

unsafe impl Send for KEvent {}
unsafe impl Sync for KEvent {}

#[pinned_drop]
impl PinnedDrop for KEvent {
    fn drop(self: Pin<&mut Self>) {
        unsafe {
            (*self.raw.get()).detach();
        }
    }
}

impl KEvent {
    pub fn new() -> Pin<Box<Self>> {
        fn init_raw() -> impl PinInit<UnsafeCell<RtEvent>> {
            let init = |slot: *mut UnsafeCell<RtEvent>| {
                let slot: *mut RtEvent = slot.cast();
                unsafe {
                    let cur_ref = &mut *slot;

                    let addr = core::ptr::addr_of!(cur_ref);
                    if let Ok(s) = CString::try_from_fmt(fmt!("{:p}", addr)) {
                        cur_ref.init(s.as_ptr() as *const i8, RT_IPC_FLAG_PRIO as u8);
                    } else {
                        let default = "default";
                        cur_ref.init(default.as_ptr() as *const i8, RT_IPC_FLAG_PRIO as u8);
                    }
                }
                Ok(())
            };
            unsafe { pin_init_from_closure(init) }
        };

        Box::pin_init(pin_init!(Self {
            raw<-init_raw(),
            pin: PhantomPinned,
        }))
        .unwrap()
    }

    pub fn send(&self, set: u32) -> Result<(), Error> {
        let result = unsafe { (*self.raw.get()).send(set) };
        if result == RT_EOK as i32 {
            Ok(())
        } else {
            Err(Error::from_errno(result))
        }
    }

    pub fn receive(&self, set: u32, option: u32, timeout: i32) -> Result<u32, Error> {
        let mut retmsg = 0u32;
        let result = unsafe { (*self.raw.get()).receive(set, option as u8, timeout, &mut retmsg) };
        if result == RT_EOK as i32 {
            Ok(retmsg)
        } else {
            Err(Error::from_errno(result))
        }
    }
}

/// Event flag raw structure
#[repr(C)]
#[pin_data]
pub struct RtEvent {
    /// Inherit from IPCObject
    #[pin]
    pub parent: IPCObject,
    /// Event flog set value
    pub set: ffi::c_uint,
}

impl_kobject!(RtEvent);

impl RtEvent {
    #[inline]
    pub fn init(&mut self, name: *const i8, flag: u8) {
        assert!((flag == RT_IPC_FLAG_FIFO as u8) || (flag == RT_IPC_FLAG_PRIO as u8));

        self.parent
            .init(ObjectClassType::ObjectClassEvent as u8, name, flag);

        self.set = 0;
    }

    #[inline]
    pub fn detach(&mut self) {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassEvent as u8);
        assert!(self.is_static_kobject());

        self.parent.wake_all();
        self.parent.parent.detach();
    }

    #[inline]
    pub fn new(name: *const i8, flag: u8) -> *mut Self {
        assert!((flag == RT_IPC_FLAG_FIFO as u8) || (flag == RT_IPC_FLAG_PRIO as u8));

        rt_debug_not_in_interrupt!();

        let event = IPCObject::new::<Self>(ObjectClassType::ObjectClassEvent as u8, name, flag);
        if !event.is_null() {
            // SAFETY: we have null protection
            unsafe {
                (*event).set = 0;
            }
        }

        event
    }

    #[inline]
    pub fn delete(&mut self) {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassEvent as u8);
        assert!(!self.is_static_kobject());

        rt_debug_not_in_interrupt!();

        self.parent.wake_all();
        self.parent.parent.delete();
    }

    pub fn send(&mut self, set: u32) -> ffi::c_long {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassEvent as u8);

        let mut need_schedule = false;
        let mut need_clear_set = 0u32;

        if set == 0 {
            return -(RT_ERROR as ffi::c_long);
        }

        self.parent.lock();

        self.set |= set;

        unsafe {
            rt_object_hook_call!(
                rt_object_put_hook,
                &mut self.parent.parent as *mut KObjectBase as *mut rt_object
            );
        }

        // SAFETY: Raw thread is not null and could be safely used.
        unsafe {
            if self.parent.has_waiting() {
                crate::list_head_for_each!(node, &self.parent.wait_list, {
                    let thread = crate::thread_list_node_entry!(node.as_ptr()) as *mut RtThread;

                    let mut status = -(RT_ERROR as ffi::c_long);
                    if (*thread).event_info as u32 & RT_EVENT_FLAG_AND > 0u32 {
                        if (*thread).event_set & self.set == (*thread).event_set {
                            status = RT_EOK as ffi::c_long;
                        }
                    } else if (*thread).event_info as u32 & RT_EVENT_FLAG_OR > 0u32 {
                        if (*thread).event_set & self.set > 0u32 {
                            (*thread).event_set = (*thread).event_set & self.set;
                            status = RT_EOK as ffi::c_long;
                        }
                    } else {
                        self.parent.unlock();
                        return -(RT_EINVAL as ffi::c_long);
                    }

                    if status == RT_EOK as ffi::c_long {
                        if (*thread).event_info as u32 & RT_EVENT_FLAG_CLEAR > 0u32 {
                            need_clear_set |= (*thread).event_set;
                        }

                        (*thread).resume();
                        (*thread).error = RT_EOK as ffi::c_long;
                        need_schedule = true;
                    }
                });

                if need_clear_set > 0 {
                    self.set &= !need_clear_set;
                }
            }
        }
        self.parent.unlock();

        if need_schedule {
            Cpu::get_current_scheduler().do_task_schedule();
        }

        RT_EOK as ffi::c_long
    }

    fn receive_internal(
        &mut self,
        set: u32,
        option: u8,
        timeout: i32,
        recved: *mut u32,
        suspend_flag: u32,
    ) -> ffi::c_long {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassEvent as u8);

        rt_debug_scheduler_available!(true);

        if set == 0 {
            return -(RT_ERROR as ffi::c_long);
        }

        let mut time_out = timeout;
        let mut status = -(RT_ERROR as ffi::c_long);
        // SAFETY: Raw thread is not null and could be safely used.
        unsafe {
            let thread = crate::current_thread_ptr!();
            if thread.is_null() {
                return -(RT_ERROR as ffi::c_long);
            }
            (*thread).error = -(RT_EINTR as ffi::c_long);
            unsafe {
                rt_object_hook_call!(
                    rt_object_trytake_hook,
                    &mut self.parent.parent as *mut KObjectBase as *mut rt_object
                );
            }
            self.parent.lock();

            if option as u32 & RT_EVENT_FLAG_AND > 0u32 {
                if (self.set & set) == set {
                    status = RT_EOK as ffi::c_long;
                }
            } else if option as u32 & RT_EVENT_FLAG_OR > 0u32 {
                if self.set & set > 0 {
                    status = RT_EOK as ffi::c_long;
                }
            } else {
                assert!(false);
            }

            if status == RT_EOK as ffi::c_long {
                (*thread).error = RT_EOK as ffi::c_long;

                if !recved.is_null() {
                    unsafe {
                        *recved = self.set & set;
                    }
                }

                (*thread).event_set = self.set & set;
                (*thread).event_info = option;

                if option as u32 & RT_EVENT_FLAG_CLEAR > 0u32 {
                    self.set &= !set;
                }
            } else if timeout == 0 {
                (*thread).error = -(RT_ETIMEOUT as ffi::c_long);
                self.parent.unlock();
                return -(RT_ETIMEOUT as ffi::c_long);
            } else {
                (*thread).event_set = set;
                (*thread).event_info = option;

                let ret = self
                    .parent
                    .wait(thread, self.parent.flag, suspend_flag as u32);
                if ret != RT_EOK as ffi::c_long {
                    self.parent.unlock();
                    return ret;
                }

                if timeout > 0 {
                    (*thread).thread_timer.timer_control(
                        RT_TIMER_CTRL_SET_TIME as u32,
                        (&mut time_out) as *mut i32 as *mut c_void,
                    );
                    (*thread).thread_timer.start();
                }

                self.parent.unlock();

                Cpu::get_current_scheduler().do_task_schedule();

                if (*thread).error != RT_EOK as ffi::c_long {
                    return (*thread).error;
                }

                self.parent.lock();

                if recved != null_mut() {
                    unsafe {
                        *recved = (*thread).event_set;
                    }
                }
            }

            self.parent.unlock();
            rt_object_hook_call!(
                rt_object_take_hook,
                &mut self.parent.parent as *mut KObjectBase as *mut rt_object
            );
            (*thread).error
        }
    }

    pub fn receive(&mut self, set: u32, option: u8, timeout: i32, recved: *mut u32) -> ffi::c_long {
        self.receive_internal(set, option, timeout, recved, RT_UNINTERRUPTIBLE as u32)
    }

    pub fn receive_interruptible(
        &mut self,
        set: u32,
        option: u8,
        timeout: i32,
        recved: *mut u32,
    ) -> ffi::c_long {
        self.receive_internal(set, option, timeout, recved, RT_INTERRUPTIBLE as u32)
    }

    pub fn receive_killable(
        &mut self,
        set: u32,
        option: u8,
        timeout: i32,
        recved: *mut u32,
    ) -> ffi::c_long {
        self.receive_internal(set, option, timeout, recved, RT_KILLABLE as u32)
    }

    pub fn control(&mut self, cmd: i32, _arg: *const c_void) -> ffi::c_long {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassEvent as u8);

        if cmd == RT_IPC_CMD_RESET as i32 {
            self.parent.lock();

            self.parent.wake_all();

            self.set = 0;

            self.parent.unlock();

            Cpu::get_current_scheduler().do_task_schedule();

            return RT_EOK as ffi::c_long;
        }

        -(RT_ERROR as ffi::c_long)
    }
}

#[cfg(feature = "RT_USING_EVENT")]
#[no_mangle]
pub unsafe extern "C" fn rt_event_init(
    event: *mut RtEvent,
    name: *const core::ffi::c_char,
    flag: rt_uint8_t,
) -> rt_err_t {
    assert!(!event.is_null());
    (*event).init(name, flag);
    RT_EOK as rt_err_t
}

#[cfg(feature = "RT_USING_EVENT")]
#[no_mangle]
pub unsafe extern "C" fn rt_event_detach(event: *mut RtEvent) -> rt_err_t {
    assert!(!event.is_null());
    (*event).detach();
    RT_EOK as rt_err_t
}

#[cfg(all(feature = "RT_USING_EVENT", feature = "RT_USING_HEAP"))]
#[no_mangle]
pub unsafe extern "C" fn rt_event_create(
    name: *const core::ffi::c_char,
    flag: rt_uint8_t,
) -> *mut RtEvent {
    RtEvent::new(name, flag)
}

#[cfg(all(feature = "RT_USING_EVENT", feature = "RT_USING_HEAP"))]
#[no_mangle]
pub unsafe extern "C" fn rt_event_delete(event: *mut RtEvent) -> rt_err_t {
    assert!(!event.is_null());
    (*event).delete();
    RT_EOK as rt_err_t
}

#[cfg(feature = "RT_USING_EVENT")]
#[no_mangle]
pub unsafe extern "C" fn rt_event_send(event: *mut RtEvent, set: rt_uint32_t) -> rt_err_t {
    assert!(!event.is_null());
    (*event).send(set)
}

#[cfg(feature = "RT_USING_EVENT")]
#[no_mangle]
pub unsafe extern "C" fn rt_event_recv(
    event: *mut RtEvent,
    set: rt_uint32_t,
    option: rt_uint8_t,
    timeout: rt_int32_t,
    recved: *mut rt_uint32_t,
) -> rt_err_t {
    assert!(!event.is_null());
    (*event).receive(set, option, timeout, recved as *mut u32)
}

#[cfg(feature = "RT_USING_EVENT")]
#[no_mangle]
pub unsafe extern "C" fn rt_event_recv_interruptible(
    event: *mut RtEvent,
    set: rt_uint32_t,
    option: rt_uint8_t,
    timeout: rt_int32_t,
    recved: *mut rt_uint32_t,
) -> rt_err_t {
    assert!(!event.is_null());
    (*event).receive_interruptible(set, option, timeout, recved as *mut u32)
}

#[cfg(feature = "RT_USING_EVENT")]
#[no_mangle]
pub unsafe extern "C" fn rt_event_recv_killable(
    event: *mut RtEvent,
    set: rt_uint32_t,
    option: rt_uint8_t,
    timeout: rt_int32_t,
    recved: *mut rt_uint32_t,
) -> rt_err_t {
    assert!(!event.is_null());
    (*event).receive_killable(set, option, timeout, recved as *mut u32)
}

#[cfg(feature = "RT_USING_EVENT")]
#[no_mangle]
pub unsafe extern "C" fn rt_event_control(
    event: *mut RtEvent,
    cmd: i32,
    _arg: *const c_void,
) -> rt_err_t {
    assert!(!event.is_null());
    (*event).control(cmd, _arg)
}

#[no_mangle]
#[allow(unused_unsafe)]
pub extern "C" fn rt_event_info() {
    let callback_forword = || {
        println!("event         set    suspend thread");
        println!("--------  ---------- --------------");
    };
    let callback = |node: &ListHead| unsafe {
        let event = &*(crate::list_head_entry!(node.as_ptr(), KObjectBase, list) as *const RtEvent);
        let _ = crate::format_name!(event.parent.parent.name.as_ptr(), 8);
        print!(" 0x{:08x} ", event.set);
        if event.parent.wait_list.is_empty() {
            println!("000");
        } else {
            print!("{}:", event.parent.wait_list.size());
            let head = &event.parent.wait_list;

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
        ObjectClassType::ObjectClassEvent as u8,
    );
}
