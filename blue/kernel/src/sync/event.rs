use crate::{
    cpu::Cpu,
    error::{code, Error},
    impl_kobject, list_head_for_each,
    object::*,
    print, println,
    rt_bindings::{
        rt_debug_not_in_interrupt, rt_debug_scheduler_available, rt_err_t, rt_int32_t, rt_object,
        rt_object_hook_call, rt_uint32_t, rt_uint8_t, RT_EINVAL, RT_EOK, RT_ERROR, RT_ETIMEOUT,
        RT_EVENT_FLAG_AND, RT_EVENT_FLAG_CLEAR, RT_EVENT_FLAG_OR, RT_INTERRUPTIBLE,
        RT_IPC_CMD_RESET, RT_IPC_FLAG_FIFO, RT_IPC_FLAG_PRIO, RT_KILLABLE, RT_TIMER_CTRL_SET_TIME,
        RT_UNINTERRUPTIBLE,
    },
    sync::ipc_common::*,
    thread::RtThread,
};

use blue_infra::list::doubly_linked_list::ListHead;
use core::{ffi::c_void, marker::PhantomPinned, ptr::null_mut};

use crate::alloc::boxed::Box;
use core::{cell::UnsafeCell, pin::Pin};
use kernel::{fmt, str::CString};
use pinned_init::{pin_data, pin_init, pin_init_from_closure, pinned_drop, InPlaceInit, PinInit};

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

                    if let Ok(s) = CString::try_from_fmt(fmt!("{:p}", slot)) {
                        cur_ref.init(s.as_ptr() as *const i8, RT_IPC_FLAG_PRIO as u8);
                    } else {
                        let default = "default";
                        cur_ref.init(default.as_ptr() as *const i8, RT_IPC_FLAG_PRIO as u8);
                    }
                }
                Ok(())
            };
            unsafe { pin_init_from_closure(init) }
        }

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
    pub(crate) parent: IPCObject,
    /// Event flog set value
    pub(crate) set: u32,
}

impl_kobject!(RtEvent);

impl RtEvent {
    #[inline]
    pub fn new(name: [i8; NAME_MAX], flag: u8) -> impl PinInit<Self> {
        assert!((flag == RT_IPC_FLAG_FIFO as u8) || (flag == RT_IPC_FLAG_PRIO as u8));

        rt_debug_not_in_interrupt!();

        pin_init!(Self {
            parent<-IPCObject::new(ObjectClassType::ObjectClassEvent as u8, name, flag),
            set: 0,
        })
    }

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
    pub fn new_raw(name: *const i8, flag: u8) -> *mut Self {
        let event = Box::pin_init(RtEvent::new(char_ptr_to_array(name), flag));
        match event {
            Ok(evt) => unsafe { Box::leak(Pin::into_inner_unchecked(evt)) },
            Err(_) => return null_mut(),
        }
    }

    #[inline]
    pub fn delete_raw(&mut self) {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassEvent as u8);
        assert!(!self.is_static_kobject());

        rt_debug_not_in_interrupt!();

        self.parent.wake_all();
        self.parent.parent.delete();
    }

    pub fn send(&mut self, set: u32) -> i32 {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassEvent as u8);

        let mut need_schedule = false;
        let mut need_clear_set = 0u32;

        if set == 0 {
            return -(RT_ERROR as i32);
        }

        self.parent.lock();

        self.set |= set;

        unsafe {
            rt_object_hook_call!(
                rt_object_put_hook,
                &mut self.parent.parent as *mut KObjectBase as *mut rt_object
            );
        }

        if self.parent.has_waiting() {
            // SAFETY: thread ensured not null
            unsafe {
                crate::list_head_for_each!(node, &self.parent.wait_list, {
                    let thread = crate::thread_list_node_entry!(node.as_ptr()) as *mut RtThread;

                    if !thread.is_null() {
                        let mut status = -(RT_ERROR as i32);
                        if (*thread).event_info.info as u32 & RT_EVENT_FLAG_AND > 0u32 {
                            if (*thread).event_info.set & self.set == (*thread).event_info.set {
                                status = RT_EOK as i32;
                            }
                        } else if (*thread).event_info.info as u32 & RT_EVENT_FLAG_OR > 0u32 {
                            if (*thread).event_info.set & self.set > 0u32 {
                                (*thread).event_info.set = (*thread).event_info.set & self.set;
                                status = RT_EOK as i32;
                            }
                        } else {
                            self.parent.unlock();
                            return -(RT_EINVAL as i32);
                        }

                        if status == RT_EOK as i32 {
                            if (*thread).event_info.info as u32 & RT_EVENT_FLAG_CLEAR > 0u32 {
                                need_clear_set |= (*thread).event_info.set;
                            }

                            (*thread).resume();
                            (*thread).error = code::EOK;
                            need_schedule = true;
                        }
                    }
                });
            }

            if need_clear_set > 0 {
                self.set &= !need_clear_set;
            }
        }

        self.parent.unlock();

        if need_schedule {
            Cpu::get_current_scheduler().do_task_schedule();
        }

        RT_EOK as i32
    }

    fn receive_internal(
        &mut self,
        set: u32,
        option: u8,
        timeout: i32,
        recved: *mut u32,
        suspend_flag: u32,
    ) -> i32 {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassEvent as u8);

        rt_debug_scheduler_available!(true);

        if set == 0 {
            return -(RT_ERROR as i32);
        }

        let mut time_out = timeout;
        let mut status = -(RT_ERROR as i32);

        let thread_ptr = crate::current_thread_ptr!();
        if thread_ptr.is_null() {
            return -(RT_ERROR as i32);
        }

        // SAFETY: thread ensured not null
        let thread = unsafe { &mut *thread_ptr };

        thread.error = code::EINTR;

        unsafe {
            rt_object_hook_call!(
                rt_object_trytake_hook,
                &mut self.parent.parent as *mut KObjectBase as *mut rt_object
            );
        }

        self.parent.lock();

        if option as u32 & RT_EVENT_FLAG_AND > 0u32 {
            if (self.set & set) == set {
                status = RT_EOK as i32;
            }
        } else if option as u32 & RT_EVENT_FLAG_OR > 0u32 {
            if self.set & set > 0 {
                status = RT_EOK as i32;
            }
        } else {
            assert!(false);
        }

        if status == RT_EOK as i32 {
            thread.error = code::EOK;

            if !recved.is_null() {
                // SAFETY: recved is null checked
                unsafe {
                    *recved = self.set & set;
                }
            }

            thread.event_info.set = self.set & set;
            thread.event_info.info = option;

            if option as u32 & RT_EVENT_FLAG_CLEAR > 0u32 {
                self.set &= !set;
            }
        } else if timeout == 0 {
            thread.error = code::ETIMEOUT;
            self.parent.unlock();
            return -(RT_ETIMEOUT as i32);
        } else {
            thread.event_info.set = set;
            thread.event_info.info = option;

            let ret = self
                .parent
                .wait(thread_ptr, self.parent.flag, suspend_flag as u32);
            if ret != RT_EOK as i32 {
                self.parent.unlock();
                return ret;
            }

            if timeout > 0 {
                thread.thread_timer.timer_control(
                    RT_TIMER_CTRL_SET_TIME as u32,
                    (&mut time_out) as *mut i32 as *mut c_void,
                );
                thread.thread_timer.start();
            }

            self.parent.unlock();

            Cpu::get_current_scheduler().do_task_schedule();

            if thread.error != code::EOK {
                return thread.error.to_errno();
            }

            self.parent.lock();

            if recved != null_mut() {
                // SAFETY: recved is null checked
                unsafe {
                    *recved = (*thread).event_info.set;
                }
            }
        }

        self.parent.unlock();

        unsafe {
            rt_object_hook_call!(
                rt_object_take_hook,
                &mut self.parent.parent as *mut KObjectBase as *mut rt_object
            );
        }

        thread.error.to_errno()
    }

    pub fn receive(&mut self, set: u32, option: u8, timeout: i32, recved: *mut u32) -> i32 {
        self.receive_internal(set, option, timeout, recved, RT_UNINTERRUPTIBLE as u32)
    }

    pub fn receive_interruptible(
        &mut self,
        set: u32,
        option: u8,
        timeout: i32,
        recved: *mut u32,
    ) -> i32 {
        self.receive_internal(set, option, timeout, recved, RT_INTERRUPTIBLE as u32)
    }

    pub fn receive_killable(
        &mut self,
        set: u32,
        option: u8,
        timeout: i32,
        recved: *mut u32,
    ) -> i32 {
        self.receive_internal(set, option, timeout, recved, RT_KILLABLE as u32)
    }

    pub fn control(&mut self, cmd: i32, _arg: *const c_void) -> i32 {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassEvent as u8);

        if cmd == RT_IPC_CMD_RESET as i32 {
            self.parent.lock();

            self.parent.wake_all();

            self.set = 0;

            self.parent.unlock();

            Cpu::get_current_scheduler().do_task_schedule();

            return RT_EOK as i32;
        }

        -(RT_ERROR as i32)
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
    RtEvent::new_raw(name, flag)
}

#[cfg(all(feature = "RT_USING_EVENT", feature = "RT_USING_HEAP"))]
#[no_mangle]
pub unsafe extern "C" fn rt_event_delete(event: *mut RtEvent) -> rt_err_t {
    assert!(!event.is_null());
    (*event).delete_raw();
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
