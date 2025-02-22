use crate::{
    cpu::Cpu,
    error::{code, Error},
    impl_kobject,
    object::*,
    sync::{ipc_common::*, wait_list::WaitMode},
    thread::{SuspendFlag, Thread},
    timer::TimerControlAction,
};
use core::{ffi::c_void, marker::PhantomPinned, ptr::null_mut};

use crate::alloc::{boxed::Box, ffi::CString, format};
use core::{cell::UnsafeCell, mem, pin::Pin};
use pinned_init::{pin_data, pin_init, pin_init_from_closure, pinned_drop, InPlaceInit, PinInit};

#[pin_data(PinnedDrop)]
pub struct KEvent {
    #[pin]
    raw: UnsafeCell<Event>,
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
        fn init_raw() -> impl PinInit<UnsafeCell<Event>> {
            let init = |slot: *mut UnsafeCell<Event>| {
                let slot: *mut Event = slot.cast();
                unsafe {
                    let cur_ref = &mut *slot;

                    if let Ok(s) = CString::new(format!("{:p}", slot)) {
                        cur_ref.init(s.as_ptr() as *const i8, WaitMode::Priority);
                    } else {
                        let default = "default";
                        cur_ref.init(default.as_ptr() as *const i8, WaitMode::Priority);
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
        unsafe { (*self.raw.get()).send(set) }
    }

    pub fn receive(&self, set: u32, option: u32, timeout: i32) -> Result<u32, Error> {
        unsafe { (*self.raw.get()).receive(set, option as u8, timeout) }
    }
}

//TODO: rewrite by struct(u32)
const EVENT_AND: u32 = 1;
const EVENT_OR: u32 = 2;
const EVENT_CLEAR: u32 = 4;

/// Event flag raw structure
#[repr(C)]
#[pin_data]
pub struct Event {
    /// Inherit from KObjectBase
    #[pin]
    pub(crate) parent: KObjectBase,
    /// Event flog set value
    pub(crate) set: u32,
    #[pin]
    /// SysQueue for Event flag
    #[pin]
    pub(crate) inner_queue: SysQueue,
}

impl_kobject!(Event);

impl Event {
    #[inline]
    pub fn new(name: [i8; NAME_MAX], wait_mode: WaitMode) -> impl PinInit<Self> {
        pin_init!(Self {
            parent<-KObjectBase::new(ObjectClassType::ObjectClassEvent as u8, name),
            set: 0,
            inner_queue<-SysQueue::new(mem::size_of::<u32>(), 1, IPC_SYS_QUEUE_STUB, wait_mode)
        })
    }

    #[inline]
    pub fn init(&mut self, name: *const i8, wait_mode: WaitMode) {
        self.parent
            .init(ObjectClassType::ObjectClassEvent as u8, name);

        self.set = 0;

        self.inner_queue.init(
            null_mut(),
            mem::size_of::<u32>(),
            1,
            IPC_SYS_QUEUE_STUB,
            wait_mode,
        );
    }

    #[inline]
    pub fn detach(&mut self) {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassEvent as u8);

        self.inner_queue.lock();
        self.inner_queue.dequeue_waiter.wake_all();
        self.inner_queue.unlock();

        if self.is_static_kobject() {
            self.parent.detach();
        }
    }

    #[inline]
    pub fn new_raw(name: *const i8, wait_mode: WaitMode) -> *mut Self {
        let event = Box::pin_init(Event::new(char_ptr_to_array(name), wait_mode));
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

        self.inner_queue.lock();
        self.inner_queue.dequeue_waiter.wake_all();
        self.inner_queue.unlock();

        self.parent.delete();
    }

    pub fn send(&mut self, set: u32) -> Result<(), Error> {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassEvent as u8);

        let mut need_schedule = false;
        let mut need_clear_set = 0u32;

        if set == 0 {
            return Err(code::ERROR);
        }

        // Critical section for event flag query and set
        {
            let _ = self.inner_queue.spinlock.acquire();

            self.set |= set;

            crate::doubly_linked_list_for_each!(
                node,
                &self.inner_queue.dequeue_waiter.wait_list,
                {
                    let thread = unsafe {
                        &mut *(crate::thread_list_node_entry!(node.as_ptr()) as *mut Thread)
                    };

                    let mut status = code::ERROR;
                    if thread.event_info.info as u32 & EVENT_AND > 0u32 {
                        if thread.event_info.set & self.set == thread.event_info.set {
                            status = code::EOK;
                        }
                    } else if thread.event_info.info as u32 & EVENT_OR > 0u32 {
                        if thread.event_info.set & self.set > 0u32 {
                            thread.event_info.set = thread.event_info.set & self.set;
                            status = code::EOK;
                        }
                    } else {
                        self.inner_queue.unlock();
                        return Err(code::EINVAL);
                    }

                    if status == code::EOK {
                        if thread.event_info.info as u32 & EVENT_CLEAR > 0u32 {
                            need_clear_set |= (*thread).event_info.set;
                        }

                        // node will be removed from working_queue by resume, so we need to get prev node
                        node = unsafe { node.prev().unwrap_unchecked().as_ref() };
                        thread.remove_thread_list_node();
                        thread.resume();
                        thread.error = code::EOK;
                        need_schedule = true;
                    }
                }
            );

            if need_clear_set > 0 {
                self.set &= !need_clear_set;
            }
        }

        if need_schedule {
            Cpu::get_current_scheduler().do_task_schedule();
        }

        Ok(())
    }

    fn receive_internal(
        &mut self,
        set: u32,
        option: u8,
        timeout: i32,
        suspend_flag: SuspendFlag,
    ) -> Result<u32, Error> {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassEvent as u8);

        crate::debug_scheduler_available!(true);

        if set == 0 {
            return Err(code::ERROR);
        }

        let mut time_out = timeout;
        let mut status = code::ERROR;

        let thread_ptr = crate::current_thread_ptr!();
        if thread_ptr.is_null() {
            return Err(code::ERROR);
        }

        // SAFETY: thread ensured not null
        let thread = unsafe { &mut *thread_ptr };
        thread.error = code::EINTR;

        // Critical section for event flag query and set
        {
            // SAFETY: recved is not used when error occurs
            #[allow(unused_mut, unused_assignments)]
            let mut recved = 0u32;
            let spin_guard = self.inner_queue.spinlock.acquire();

            if option as u32 & EVENT_AND > 0u32 {
                if (self.set & set) == set {
                    status = code::EOK;
                }
            } else if option as u32 & EVENT_OR > 0u32 {
                if self.set & set > 0 {
                    status = code::EOK;
                }
            } else {
                return Err(code::EINVAL);
            }

            if status == code::EOK {
                thread.error = code::EOK;
                recved = self.set & set;
                thread.event_info.set = self.set & set;
                thread.event_info.info = option;

                if option as u32 & EVENT_CLEAR > 0u32 {
                    self.set &= !set;
                }
            } else if timeout == 0 {
                thread.error = code::ETIMEDOUT;
                return Err(code::ETIMEDOUT);
            } else {
                thread.event_info.set = set;
                thread.event_info.info = option;

                self.inner_queue.dequeue_waiter.wait(thread, suspend_flag)?;

                if timeout > 0 {
                    thread.thread_timer.timer_control(
                        TimerControlAction::SetTime,
                        (&mut time_out) as *mut i32 as *mut c_void,
                    );
                    thread.thread_timer.start();
                }

                drop(spin_guard);

                Cpu::get_current_scheduler().do_task_schedule();

                if thread.error != code::EOK {
                    return Err(thread.error);
                }
                {
                    let _ = self.inner_queue.spinlock.acquire();
                    recved = (*thread).event_info.set;
                }
            }
            Ok(recved)
        }
    }

    #[inline]
    pub fn receive(&mut self, set: u32, option: u8, timeout: i32) -> Result<u32, Error> {
        self.receive_internal(set, option, timeout, SuspendFlag::Uninterruptible)
    }

    #[inline]
    pub fn receive_interruptible(
        &mut self,
        set: u32,
        option: u8,
        timeout: i32,
    ) -> Result<u32, Error> {
        self.receive_internal(set, option, timeout, SuspendFlag::Interruptible)
    }

    #[inline]
    pub fn receive_killable(&mut self, set: u32, option: u8, timeout: i32) -> Result<u32, Error> {
        self.receive_internal(set, option, timeout, SuspendFlag::Killable)
    }

    pub fn reset(&mut self) -> Result<(), Error> {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassEvent as u8);

        // Critical section for event reset
        {
            let _ = self.inner_queue.spinlock.acquire();
            self.inner_queue.dequeue_waiter.wake_all();
            self.set = 0;
        }
        Cpu::get_current_scheduler().do_task_schedule();

        Ok(())
    }
}

/// bindgen for Event
#[allow(improper_ctypes_definitions)]
#[no_mangle]
pub extern "C" fn bindgen_event(_event: Event) {
    0;
}
