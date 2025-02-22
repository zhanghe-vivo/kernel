use crate::{
    clock::get_tick,
    cpu::Cpu,
    error::{code, Error},
    impl_kobject,
    object::*,
    sync::{ipc_common::*, wait_list::WaitMode},
    thread::SuspendFlag,
    timer::TimerControlAction,
};
#[allow(unused_imports)]
use core::{
    cell::UnsafeCell,
    ffi,
    ffi::{c_char, c_void},
    marker::PhantomPinned,
    mem,
    mem::MaybeUninit,
    ptr::null_mut,
};

use crate::alloc::{boxed::Box, ffi::CString, format};
use core::pin::Pin;
use pinned_init::{pin_data, pin_init, pin_init_from_closure, pinned_drop, InPlaceInit, PinInit};

#[pin_data(PinnedDrop)]
pub struct KMailbox {
    #[pin]
    raw: UnsafeCell<Mailbox>,
    #[pin]
    pin: PhantomPinned,
}

unsafe impl Send for KMailbox {}
unsafe impl Sync for KMailbox {}

#[pinned_drop]
impl PinnedDrop for KMailbox {
    fn drop(self: Pin<&mut Self>) {
        unsafe {
            (*self.raw.get()).detach();
        }
    }
}

impl KMailbox {
    pub fn new(size: usize) -> Pin<Box<Self>> {
        fn init_raw(size: usize) -> impl PinInit<UnsafeCell<Mailbox>> {
            let init = move |slot: *mut UnsafeCell<Mailbox>| {
                let slot: *mut Mailbox = slot.cast();
                unsafe {
                    let cur_ref = &mut *slot;

                    if let Ok(s) = CString::new(format!("{:p}", slot)) {
                        cur_ref.parent.init(
                            ObjectClassType::ObjectClassMailBox as u8,
                            s.as_ptr() as *const i8,
                        );
                    } else {
                        let default = "default";
                        cur_ref.parent.init(
                            ObjectClassType::ObjectClassMailBox as u8,
                            default.as_ptr() as *const i8,
                        );
                    }

                    cur_ref.inner_queue.init(
                        null_mut(),
                        mem::size_of::<usize>(),
                        size,
                        0,
                        WaitMode::Fifo,
                    );
                }
                Ok(())
            };
            unsafe { pin_init_from_closure(init) }
        }

        Box::pin_init(pin_init!(Self {
            raw<-init_raw(size),
            pin: PhantomPinned,
        }))
        .unwrap()
    }

    pub fn send(&self, set: usize) -> Result<(), Error> {
        unsafe { (*self.raw.get()).send(set) }
    }

    pub fn receive(&self, timeout: i32) -> Result<usize, Error> {
        unsafe { (*self.raw.get()).receive(timeout) }
    }
}

/// Mailbox raw structure
#[repr(C)]
#[pin_data]
pub struct Mailbox {
    /// Inherit from KObjectBase
    #[pin]
    pub(crate) parent: KObjectBase,
    #[pin]
    /// SysQueue for mailbox
    #[pin]
    pub(crate) inner_queue: SysQueue,
}

impl_kobject!(Mailbox);

impl Mailbox {
    #[inline]
    pub fn new(name: [i8; NAME_MAX], size: usize, wait_mode: WaitMode) -> impl PinInit<Self> {
        pin_init!(Self {
            parent<-KObjectBase::new(ObjectClassType::ObjectClassMailBox as u8, name),
            inner_queue<-SysQueue::new(mem::size_of::<usize>(), size, IPC_SYS_QUEUE_FIFO, wait_mode),
        })
    }

    #[inline]
    pub fn init(&mut self, name: *const i8, buffer: *mut u8, size: usize, wait_mode: WaitMode) {
        self.parent
            .init(ObjectClassType::ObjectClassMailBox as u8, name);

        self.inner_queue.init(
            buffer,
            mem::size_of::<usize>(),
            size,
            IPC_SYS_QUEUE_FIFO,
            wait_mode,
        );
    }

    #[inline]
    pub fn detach(&mut self) {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassMailBox as u8);

        self.inner_queue.lock();
        self.inner_queue.dequeue_waiter.wake_all();
        self.inner_queue.enqueue_waiter.wake_all();
        self.inner_queue.unlock();

        if self.is_static_kobject() {
            self.parent.detach();
        }
    }

    #[inline]
    pub fn new_raw(name: *const i8, size: usize, wait_mode: WaitMode) -> *mut Self {
        let mailbox = Box::pin_init(Mailbox::new(char_ptr_to_array(name), size, wait_mode));
        match mailbox {
            Ok(mb) => unsafe { Box::leak(Pin::into_inner_unchecked(mb)) },
            Err(_) => return null_mut(),
        }
    }

    #[inline]
    pub fn delete_raw(&mut self) {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassMailBox as u8);
        assert!(!self.is_static_kobject());

        crate::debug_not_in_interrupt!();

        self.inner_queue.lock();
        self.inner_queue.dequeue_waiter.wake_all();
        self.inner_queue.enqueue_waiter.wake_all();
        self.inner_queue.unlock();

        self.parent.delete();
    }

    fn send_wait_internal(
        &mut self,
        value: usize,
        timeout: i32,
        suspend_flag: SuspendFlag,
    ) -> Result<(), Error> {
        let mut timeout = timeout;
        #[allow(unused_variables)]
        let scheduler = timeout != 0;
        crate::debug_scheduler_available!(scheduler);

        let mut tick_delta = 0;
        let thread_ptr = crate::current_thread_ptr!();
        if thread_ptr.is_null() {
            return Err(code::ERROR);
        }

        // SAFETY: thread_ptr is null checked
        let thread = unsafe { &mut *thread_ptr };

        self.inner_queue.lock();

        if self.inner_queue.is_full() && timeout == 0 {
            self.inner_queue.unlock();
            return Err(code::ENOSPC);
        }

        while self.inner_queue.is_full() {
            thread.error = code::EINTR;

            if timeout == 0 {
                self.inner_queue.unlock();
                return Err(code::ENOSPC);
            }

            self.inner_queue
                .enqueue_waiter
                .wait(thread, suspend_flag)
                .map_err(|e| {
                    self.inner_queue.unlock();
                    e
                })?;

            if timeout > 0 {
                tick_delta = get_tick();

                thread.thread_timer.timer_control(
                    TimerControlAction::SetTime,
                    (&mut timeout) as *mut i32 as *mut c_void,
                );
                thread.thread_timer.start();
            }

            self.inner_queue.unlock();

            Cpu::get_current_scheduler().do_task_schedule();

            if thread.error != code::EOK {
                return Err(thread.error);
            }

            self.inner_queue.lock();

            if timeout > 0 {
                tick_delta = get_tick() - tick_delta;
                timeout -= tick_delta as i32;
                if timeout < 0 {
                    timeout = 0;
                }
            }
        }

        if self
            .inner_queue
            .push_fifo(&value as *const usize as *const u8, mem::size_of::<usize>())
            == 0
        {
            self.inner_queue.unlock();
            return Err(code::ENOSPC);
        }

        if self.inner_queue.dequeue_waiter.wake() {
            self.inner_queue.unlock();

            Cpu::get_current_scheduler().do_task_schedule();

            return Ok(());
        }

        self.inner_queue.unlock();

        Ok(())
    }

    pub fn send_wait(&mut self, value: usize, timeout: i32) -> Result<(), Error> {
        self.send_wait_internal(value, timeout, SuspendFlag::Uninterruptible)
    }

    pub fn send_wait_interruptible(&mut self, value: usize, timeout: i32) -> Result<(), Error> {
        self.send_wait_internal(value, timeout, SuspendFlag::Interruptible)
    }

    pub fn send_wait_killable(&mut self, value: usize, timeout: i32) -> Result<(), Error> {
        self.send_wait_internal(value, timeout, SuspendFlag::Killable)
    }

    pub fn send(&mut self, value: usize) -> Result<(), Error> {
        self.send_wait(value, 0)
    }

    pub fn send_interruptible(&mut self, value: usize) -> Result<(), Error> {
        self.send_wait_interruptible(value, 0)
    }

    pub fn send_killable(&mut self, value: usize) -> Result<(), Error> {
        self.send_wait_killable(value, 0)
    }

    pub fn urgent(&mut self, value: usize) -> Result<(), Error> {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassMailBox as u8);

        self.inner_queue.lock();

        if self.inner_queue.is_full() {
            self.inner_queue.unlock();
            return Err(code::ENOSPC);
        }

        self.inner_queue
            .urgent_fifo(&value as *const usize as *const u8, mem::size_of::<usize>());

        if self.inner_queue.dequeue_waiter.wake() {
            self.inner_queue.unlock();
            Cpu::get_current_scheduler().do_task_schedule();
            return Ok(());
        }

        self.inner_queue.unlock();
        Ok(())
    }

    fn receive_internal(
        &mut self,
        timeout: i32,
        suspend_flag: SuspendFlag,
    ) -> Result<usize, Error> {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassMailBox as u8);

        let mut timeout = timeout;
        #[allow(unused_variables)]
        let scheduler = timeout != 0;
        crate::debug_scheduler_available!(scheduler);

        let mut tick_delta = 0;
        let thread_ptr = unsafe { crate::current_thread!().unwrap().as_mut() };
        let mut value: usize = 0;

        self.inner_queue.lock();

        if self.inner_queue.is_empty() && timeout == 0 {
            self.inner_queue.unlock();
            return Err(code::ETIMEDOUT);
        }

        let thread = &mut *thread_ptr;
        while self.inner_queue.is_empty() {
            thread.error = code::EINTR;

            if timeout == 0 {
                self.inner_queue.unlock();
                thread.error = code::ETIMEDOUT;

                return Err(code::ETIMEDOUT);
            }

            self.inner_queue
                .dequeue_waiter
                .wait(thread, suspend_flag)
                .map_err(|e| {
                    self.inner_queue.unlock();
                    e
                })?;

            if timeout > 0 {
                tick_delta = Cpu::get_by_id(0).tick_load();

                thread.thread_timer.timer_control(
                    TimerControlAction::SetTime,
                    (&mut timeout) as *mut i32 as *mut c_void,
                );
                thread.thread_timer.start();
            }

            self.inner_queue.unlock();

            Cpu::get_current_scheduler().do_task_schedule();

            if thread.error != code::EOK {
                return Err(thread.error);
            }

            self.inner_queue.lock();

            if timeout > 0 {
                tick_delta = Cpu::get_by_id(0).tick_load() - tick_delta;
                timeout -= tick_delta as i32;
                if timeout < 0 {
                    timeout = 0;
                }
            }
        }

        self.inner_queue
            .pop_fifo(&mut value as *mut usize as *mut u8, mem::size_of::<usize>());

        if self.inner_queue.enqueue_waiter.wake() {
            self.inner_queue.unlock();
            Cpu::get_current_scheduler().do_task_schedule();
            return Ok(value);
        }

        self.inner_queue.unlock();
        return Ok(value);
    }

    pub fn receive(&mut self, timeout: i32) -> Result<usize, Error> {
        self.receive_internal(timeout, SuspendFlag::Uninterruptible)
    }

    pub fn receive_interruptible(&mut self, timeout: i32) -> Result<usize, Error> {
        self.receive_internal(timeout, SuspendFlag::Interruptible)
    }

    pub fn receive_killable(&mut self, timeout: i32) -> Result<usize, Error> {
        self.receive_internal(timeout, SuspendFlag::Killable)
    }

    pub fn reset(&mut self) -> Result<(), Error> {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassMailBox as u8);

        {
            let _ = self.inner_queue.spinlock.acquire();
            self.inner_queue.dequeue_waiter.wake_all();
            self.inner_queue.enqueue_waiter.wake_all();
            self.inner_queue.item_in_queue = 0;
            self.inner_queue.read_pos = 0;
            self.inner_queue.write_pos = 0;
        }

        Cpu::get_current_scheduler().do_task_schedule();

        Ok(())
    }
}

/// bindgen for Mailbox
#[allow(improper_ctypes_definitions)]
#[no_mangle]
pub extern "C" fn bindgen_mb(_mb: Mailbox) {
    0;
}
