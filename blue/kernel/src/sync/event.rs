use crate::{
    cpu::Cpu,
    error::{code, Error},
    impl_kobject,
    object::*,
    sync::ipc_common::*,
    thread::{RtThread, SuspendFlag},
    timer::TimerControlAction,
};
use blue_infra::list::doubly_linked_list::ListHead;
use core::{ffi::c_void, marker::PhantomPinned, ptr::null_mut};

use crate::alloc::boxed::Box;
use core::{cell::UnsafeCell, mem, pin::Pin};
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
                        cur_ref.init(s.as_ptr() as *const i8, IPC_WAIT_MODE_PRIO as u8);
                    } else {
                        let default = "default";
                        cur_ref.init(default.as_ptr() as *const i8, IPC_WAIT_MODE_PRIO as u8);
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
        if result == code::EOK.to_errno() {
            Ok(())
        } else {
            Err(Error::from_errno(result))
        }
    }

    pub fn receive(&self, set: u32, option: u32, timeout: i32) -> Result<u32, Error> {
        let mut retmsg = 0u32;
        let result = unsafe { (*self.raw.get()).receive(set, option as u8, timeout, &mut retmsg) };
        if result == code::EOK.to_errno() {
            Ok(retmsg)
        } else {
            Err(Error::from_errno(result))
        }
    }
}

//TODO: rewrite by struct(u32)
const EVENT_AND: u32 = 1;
const EVENT_OR: u32 = 2;
const EVENT_CLEAR: u32 = 4;

/// Event flag raw structure
#[repr(C)]
#[pin_data]
pub struct RtEvent {
    /// Inherit from KObjectBase
    #[pin]
    pub(crate) parent: KObjectBase,
    /// Event flog set value
    pub(crate) set: u32,
    #[pin]
    /// SysQueue for Event flag
    #[pin]
    pub(crate) inner_queue: RtSysQueue,
}

impl_kobject!(RtEvent);

impl RtEvent {
    #[inline]
    pub fn new(name: [i8; NAME_MAX], flag: u8) -> impl PinInit<Self> {
        assert!((flag == IPC_WAIT_MODE_FIFO as u8) || (flag == IPC_WAIT_MODE_PRIO as u8));

        crate::debug_not_in_interrupt!();

        pin_init!(Self {
            parent<-KObjectBase::new(ObjectClassType::ObjectClassEvent as u8, name),
            set: 0,
            inner_queue<-RtSysQueue::new(mem::size_of::<u32>(), 1, IPC_SYS_QUEUE_STUB, flag as u32)
        })
    }

    #[inline]
    pub fn init(&mut self, name: *const i8, flag: u8) {
        assert!((flag == IPC_WAIT_MODE_FIFO as u8) || (flag == IPC_WAIT_MODE_PRIO as u8));

        self.parent
            .init(ObjectClassType::ObjectClassEvent as u8, name);

        self.set = 0;

        self.inner_queue.init(
            null_mut(),
            mem::size_of::<u32>(),
            1,
            IPC_SYS_QUEUE_STUB,
            flag as u32,
        );
    }

    #[inline]
    pub fn detach(&mut self) {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassEvent as u8);

        self.inner_queue.dequeue_waiter.inner_locked_wake_all();

        if self.is_static_kobject() {
            self.parent.detach();
        }
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

        crate::debug_not_in_interrupt!();

        self.inner_queue.dequeue_waiter.inner_locked_wake_all();
        self.parent.delete();
    }

    pub fn send(&mut self, set: u32) -> i32 {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassEvent as u8);

        let mut need_schedule = false;
        let mut need_clear_set = 0u32;

        if set == 0 {
            return code::ERROR.to_errno();
        }

        self.inner_queue.lock();

        self.set |= set;

        if !self.inner_queue.dequeue_waiter.is_empty() {
            crate::list_head_for_each!(node, &self.inner_queue.dequeue_waiter.working_queue, {
                let thread_ptr =
                    unsafe { crate::thread_list_node_entry!(node.as_ptr()) as *mut RtThread };

                if !thread_ptr.is_null() {
                    let thread = unsafe { &mut *thread_ptr };
                    let mut status = code::ERROR.to_errno();
                    if thread.event_info.info as u32 & EVENT_AND > 0u32 {
                        if thread.event_info.set & self.set == thread.event_info.set {
                            status = code::EOK.to_errno();
                        }
                    } else if thread.event_info.info as u32 & EVENT_OR > 0u32 {
                        if thread.event_info.set & self.set > 0u32 {
                            thread.event_info.set = thread.event_info.set & self.set;
                            status = code::EOK.to_errno();
                        }
                    } else {
                        self.inner_queue.unlock();
                        return code::EINVAL.to_errno();
                    }

                    if status == code::EOK.to_errno() {
                        if thread.event_info.info as u32 & EVENT_CLEAR > 0u32 {
                            need_clear_set |= (*thread).event_info.set;
                        }

                        thread.resume();
                        thread.error = code::EOK;
                        need_schedule = true;
                    }
                }
            });

            if need_clear_set > 0 {
                self.set &= !need_clear_set;
            }
        }

        self.inner_queue.unlock();

        if need_schedule {
            Cpu::get_current_scheduler().do_task_schedule();
        }

        code::EOK.to_errno()
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

        crate::debug_scheduler_available!(true);

        if set == 0 {
            return code::ERROR.to_errno();
        }

        let mut time_out = timeout;
        let mut status = code::ERROR.to_errno();

        let thread_ptr = crate::current_thread_ptr!();
        if thread_ptr.is_null() {
            return code::ERROR.to_errno();
        }

        // SAFETY: thread ensured not null
        let thread = unsafe { &mut *thread_ptr };

        thread.error = code::EINTR;

        self.inner_queue.lock();

        if option as u32 & EVENT_AND > 0u32 {
            if (self.set & set) == set {
                status = code::EOK.to_errno();
            }
        } else if option as u32 & EVENT_OR > 0u32 {
            if self.set & set > 0 {
                status = code::EOK.to_errno();
            }
        } else {
            assert!(false);
        }

        if status == code::EOK.to_errno() {
            thread.error = code::EOK;

            if !recved.is_null() {
                // SAFETY: recved is null checked
                unsafe {
                    *recved = self.set & set;
                }
            }

            thread.event_info.set = self.set & set;
            thread.event_info.info = option;

            if option as u32 & EVENT_CLEAR > 0u32 {
                self.set &= !set;
            }
        } else if timeout == 0 {
            thread.error = code::ETIMEDOUT;
            self.inner_queue.unlock();
            return code::ETIMEDOUT.to_errno();
        } else {
            thread.event_info.set = set;
            thread.event_info.info = option;

            let ret = self
                .inner_queue
                .dequeue_waiter
                .wait(thread, suspend_flag as u32);
            if ret != code::EOK.to_errno() {
                self.inner_queue.unlock();
                return ret;
            }

            if timeout > 0 {
                thread.thread_timer.timer_control(
                    TimerControlAction::SetTime,
                    (&mut time_out) as *mut i32 as *mut c_void,
                );
                thread.thread_timer.start();
            }

            self.inner_queue.unlock();

            Cpu::get_current_scheduler().do_task_schedule();

            if thread.error != code::EOK {
                return thread.error.to_errno();
            }

            self.inner_queue.lock();

            if recved != null_mut() {
                // SAFETY: recved is null checked
                unsafe {
                    *recved = (*thread).event_info.set;
                }
            }
        }

        self.inner_queue.unlock();

        thread.error.to_errno()
    }

    pub fn receive(&mut self, set: u32, option: u8, timeout: i32, recved: *mut u32) -> i32 {
        self.receive_internal(
            set,
            option,
            timeout,
            recved,
            SuspendFlag::Uninterruptible as u32,
        )
    }

    pub fn receive_interruptible(
        &mut self,
        set: u32,
        option: u8,
        timeout: i32,
        recved: *mut u32,
    ) -> i32 {
        self.receive_internal(
            set,
            option,
            timeout,
            recved,
            SuspendFlag::Interruptible as u32,
        )
    }

    pub fn receive_killable(
        &mut self,
        set: u32,
        option: u8,
        timeout: i32,
        recved: *mut u32,
    ) -> i32 {
        self.receive_internal(set, option, timeout, recved, SuspendFlag::Killable as u32)
    }

    pub fn control(&mut self, cmd: i32, _arg: *const c_void) -> i32 {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassEvent as u8);

        if cmd == IPC_CMD_RESET as i32 {
            self.inner_queue.lock();

            self.inner_queue.dequeue_waiter.inner_locked_wake_all();

            self.set = 0;

            self.inner_queue.unlock();

            Cpu::get_current_scheduler().do_task_schedule();

            return code::EOK.to_errno();
        }

        code::ERROR.to_errno()
    }
}

/// bindgen for RtEvent
#[allow(improper_ctypes_definitions)]
#[no_mangle]
pub extern "C" fn bindgen_event(_event: RtEvent) {
    0;
}
