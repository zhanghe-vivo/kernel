use crate::{
    allocator::align_up_size,
    clock::get_tick,
    cpu::Cpu,
    error::{code, Error},
    impl_kobject,
    object::*,
    sync::{ipc_common::*, wait_list::WaitMode},
    thread::SuspendFlag,
    timer::TimerControlAction,
};
use blue_kconfig::ALIGN_SIZE;
#[allow(unused_imports)]
use core::{
    alloc::AllocError,
    cell::UnsafeCell,
    ffi::{self, c_char, c_void},
    marker::PhantomPinned,
    mem,
    mem::MaybeUninit,
    ptr::{null_mut, NonNull},
    slice,
};

use crate::alloc::{boxed::Box, ffi::CString, format};
use cfg_if;
use core::pin::Pin;
use pinned_init::{pin_data, pin_init, pin_init_from_closure, pinned_drop, InPlaceInit, PinInit};

#[pin_data(PinnedDrop)]
pub struct KMessageQueue {
    #[pin]
    raw: UnsafeCell<MessageQueue>,
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
        fn init_raw(msg_size: usize, max_msgs: usize) -> impl PinInit<UnsafeCell<MessageQueue>> {
            let init = move |slot: *mut UnsafeCell<MessageQueue>| {
                let slot: *mut MessageQueue = slot.cast();
                unsafe {
                    let cur_ref = &mut *slot;

                    if let Ok(s) = CString::new(format!("{:p}", slot)) {
                        cur_ref.init(
                            s.as_ptr() as *const i8,
                            null_mut(),
                            msg_size,
                            max_msgs,
                            IPC_SYS_QUEUE_FIFO as u8,
                            WaitMode::Fifo,
                        );
                    } else {
                        let default = "default";
                        cur_ref.init(
                            default.as_ptr() as *const i8,
                            null_mut(),
                            msg_size,
                            max_msgs,
                            IPC_SYS_QUEUE_FIFO as u8,
                            WaitMode::Fifo,
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

        unsafe { (*self.raw.get()).send(msg_ptr, size) }
    }

    pub fn receive(&self, timeout: i32) -> Result<Box<[u8]>, Error> {
        let buffer = null_mut();
        let size = 0 as usize;
        match unsafe { (*self.raw.get()).receive(buffer, size, timeout) } {
            Ok(size) => {
                if buffer.is_null() || size == 0 {
                    return Ok(Box::new([]));
                }
                unsafe {
                    let slice_ptr = slice::from_raw_parts_mut(buffer, size);
                    Ok(Box::from_raw(slice_ptr))
                }
            }
            Err(e) => Err(e),
        }
    }
}

/// MessageQueue raw structure
#[repr(C)]
#[pin_data]
pub struct MessageQueue {
    /// Inherit from KObjectBase
    #[pin]
    pub(crate) parent: KObjectBase,
    /// SysQueue for mailbox
    #[pin]
    pub inner_queue: SysQueue,
}

impl_kobject!(MessageQueue);

impl MessageQueue {
    pub fn new(
        name: [i8; NAME_MAX],
        msg_size: u16,
        max_msgs: u16,
        working_mode: u8,
        waiting_mode: WaitMode,
    ) -> Result<Pin<Box<Self>>, AllocError> {
        crate::debug_not_in_interrupt!();
        Box::pin_init(pin_init!(Self {
            parent<-KObjectBase::new(ObjectClassType::ObjectClassMessageQueue as u8, name),
            inner_queue<-SysQueue::new(msg_size as usize, max_msgs as usize, working_mode as u32, waiting_mode),
        }))
    }

    #[inline]
    pub fn init(
        &mut self,
        name: *const i8,
        buffer: *mut u8,
        item_size: usize,
        buffer_size: usize,
        working_mode: u8,
        waiting_mode: WaitMode,
    ) -> Error {
        self.parent
            .init(ObjectClassType::ObjectClassMessageQueue as u8, name);

        if buffer.is_null() || item_size == 0 || buffer_size == 0 {
            return code::EINVAL;
        }

        #[allow(unused_assignments)]
        let mut max_count = 1;
        if working_mode == IPC_SYS_QUEUE_FIFO as u8 {
            max_count = buffer_size / item_size;
        } else if working_mode == IPC_SYS_QUEUE_PRIO as u8 {
            let item_align_size = align_up_size(item_size as usize, ALIGN_SIZE as usize);
            max_count =
                buffer_size as usize / (item_align_size + mem::size_of::<SysQueueItemHeader>());
        } else {
            return code::EINVAL;
        }

        if max_count == 0 {
            return code::EINVAL;
        }

        self.inner_queue.init(
            buffer,
            item_size,
            max_count,
            working_mode as u32,
            waiting_mode,
        )
    }

    #[inline]
    pub fn detach(&mut self) {
        assert_eq!(
            self.type_name(),
            ObjectClassType::ObjectClassMessageQueue as u8
        );

        self.inner_queue.lock();
        self.inner_queue.dequeue_waiter.wake_all();
        self.inner_queue.enqueue_waiter.wake_all();
        self.inner_queue.unlock();

        if self.is_static_kobject() {
            self.parent.detach();
        }
    }

    #[inline]
    pub fn new_raw(
        name: *const i8,
        msg_size: usize,
        max_msgs: usize,
        working_mode: u8,
        waiting_mode: WaitMode,
    ) -> *mut MessageQueue {
        let message_queue = MessageQueue::new(
            char_ptr_to_array(name),
            msg_size as u16,
            max_msgs as u16,
            working_mode,
            waiting_mode,
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

        crate::debug_not_in_interrupt!();

        self.inner_queue.lock();
        self.inner_queue.dequeue_waiter.wake_all();
        self.inner_queue.enqueue_waiter.wake_all();
        self.inner_queue.unlock();

        self.parent.delete();
    }

    #[allow(unused_variables)]
    fn send_wait_internal(
        &mut self,
        buffer: *const u8,
        size: usize,
        _prio: i32,
        timeout: i32,
        suspend_flag: SuspendFlag,
    ) -> Result<(), Error> {
        let mut timeout = timeout;

        assert_eq!(
            self.type_name(),
            ObjectClassType::ObjectClassMessageQueue as u8
        );

        assert!(size != 0);

        #[allow(unused_variables)]
        let scheduler = timeout != 0;
        crate::debug_scheduler_available!(scheduler);

        if size > self.inner_queue.item_size {
            return Err(code::ERROR);
        }

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

        cfg_if::cfg_if! {
            if #[cfg(feature = "messagequeue_priority")] {
                self.inner_queue.push_prio(buffer, size, prio);
            } else {
                self.inner_queue.push_fifo(buffer, size);
            }
        }

        if self.inner_queue.dequeue_waiter.wake() {
            self.inner_queue.unlock();
            Cpu::get_current_scheduler().do_task_schedule();
            return Ok(());
        }

        self.inner_queue.unlock();
        Ok(())
    }

    pub fn send(&mut self, buffer: *const u8, size: usize) -> Result<(), Error> {
        self.send_wait(buffer, size, 0)
    }

    pub fn send_interruptible(&mut self, buffer: *const u8, size: usize) -> Result<(), Error> {
        self.send_wait_interruptible(buffer, size, 0)
    }

    pub fn send_killable(&mut self, buffer: *const u8, size: usize) -> Result<(), Error> {
        self.send_wait_killable(buffer, size, 0)
    }

    pub fn send_wait(&mut self, buffer: *const u8, size: usize, timeout: i32) -> Result<(), Error> {
        self.send_wait_internal(buffer, size, 0, timeout, SuspendFlag::Uninterruptible)
    }

    pub fn send_wait_interruptible(
        &mut self,
        buffer: *const u8,
        size: usize,
        timeout: i32,
    ) -> Result<(), Error> {
        self.send_wait_internal(buffer, size, 0, timeout, SuspendFlag::Interruptible)
    }

    pub fn send_wait_killable(
        &mut self,
        buffer: *const u8,
        size: usize,
        timeout: i32,
    ) -> Result<(), Error> {
        self.send_wait_internal(buffer, size, 0, timeout, SuspendFlag::Killable)
    }

    pub fn urgent(&mut self, buffer: *const u8, size: usize) -> i32 {
        assert_eq!(
            self.type_name(),
            ObjectClassType::ObjectClassMessageQueue as u8
        );
        assert!(!buffer.is_null());
        assert!(size != 0);

        if size > self.inner_queue.item_size {
            return code::ERROR.to_errno();
        }

        #[allow(unused_assignments)]
        let mut urgent_size = 0;
        cfg_if::cfg_if! {
            if #[cfg(feature = "messagequeue_priority")] {
                urgent_size = self.inner_queue.urgent_prio(buffer, size);
            } else {
                urgent_size = self.inner_queue.urgent_fifo(buffer, size);
            }
        }

        if urgent_size > 0 {
            code::EOK.to_errno()
        } else {
            code::ERROR.to_errno()
        }
    }

    #[allow(unused_variables)]
    fn receive_internal(
        &mut self,
        buffer: *mut u8,
        size: usize,
        _prio: *mut i32,
        timeout: i32,
        suspend_flag: SuspendFlag,
    ) -> Result<usize, Error> {
        let mut timeout = timeout;

        assert_eq!(
            self.type_name(),
            ObjectClassType::ObjectClassMessageQueue as u8
        );
        assert!(!buffer.is_null());
        assert!(size != 0);

        #[allow(unused_variables)]
        let scheduler = timeout != 0;
        crate::debug_scheduler_available!(scheduler);

        let thread_ptr = crate::current_thread_ptr!();

        if thread_ptr.is_null() {
            return Err(code::ERROR);
        }

        // SAFETY: thread_ptr is null checked
        let thread = unsafe { &mut *thread_ptr };

        self.inner_queue.lock();

        if self.inner_queue.is_empty() && timeout == 0 {
            self.inner_queue.unlock();
            return Err(code::ETIMEDOUT);
        }

        while self.inner_queue.is_empty() {
            thread.error = code::EINTR;

            if timeout == 0 {
                self.inner_queue.unlock();
                thread.error = code::ETIMEDOUT;
                return Err(thread.error);
            }

            self.inner_queue
                .dequeue_waiter
                .wait(thread, suspend_flag)
                .map_err(|e| {
                    self.inner_queue.unlock();
                    e
                })?;

            let mut tick_delta: u32 = 0;
            if timeout > 0 {
                tick_delta = get_tick();

                thread.thread_timer.timer_control(
                    TimerControlAction::SetTime,
                    (&mut timeout) as *mut i32 as *mut c_void,
                );
                (*thread).thread_timer.start();
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

        cfg_if::cfg_if! {
            if #[cfg(feature = "messagequeue_priority")] {
                self.inner_queue.pop_prio(buffer, size, prio);
            } else {
                self.inner_queue.pop_fifo(buffer, size);
            }
        }

        if self.inner_queue.enqueue_waiter.wake() {
            self.inner_queue.unlock();
            Cpu::get_current_scheduler().do_task_schedule();
            return Ok(size);
        }

        self.inner_queue.unlock();
        Ok(size)
    }

    pub fn receive(&mut self, buffer: *mut u8, size: usize, timeout: i32) -> Result<usize, Error> {
        self.receive_internal(
            buffer,
            size,
            null_mut(),
            timeout,
            SuspendFlag::Uninterruptible,
        )
    }

    pub fn receive_interruptible(
        &mut self,
        buffer: *mut u8,
        size: usize,
        timeout: i32,
    ) -> Result<usize, Error> {
        self.receive_internal(
            buffer,
            size,
            null_mut(),
            timeout,
            SuspendFlag::Interruptible,
        )
    }

    pub fn receive_killable(
        &mut self,
        buffer: *mut u8,
        size: usize,
        timeout: i32,
    ) -> Result<usize, Error> {
        self.receive_internal(buffer, size, null_mut(), timeout, SuspendFlag::Killable)
    }

    pub fn reset(&mut self) -> Result<(), Error> {
        assert_eq!(
            self.type_name(),
            ObjectClassType::ObjectClassMessageQueue as u8
        );

        let spin_guard = self.inner_queue.spinlock.acquire();

        self.inner_queue.dequeue_waiter.wake_all();
        self.inner_queue.enqueue_waiter.wake_all();

        cfg_if::cfg_if! {
            if #[cfg(feature = "messagequeue_priority")] {
                while !self.inner_queue.head.is_none() {
                    let hdr = self.inner_queue.head.unwrap().as_ptr() as *mut SysQueueItemHeader;
                    let next_head = unsafe { (*hdr).next as *mut u8 };
                    if next_head.is_null() {
                        self.inner_queue.head = None;
                    } else {
                        self.inner_queue.head =
                        unsafe { Some(NonNull::new_unchecked(next_head)) };

                    }

                    if !self.inner_queue.tail.is_none() {
                        if self.inner_queue.tail.unwrap().as_ptr() == hdr as *mut u8 {
                            self.inner_queue.tail = None;
                        }
                    }

                    if !self.inner_queue.free.is_none() {
                        unsafe { (*hdr).next =
                        self.inner_queue.free.unwrap().as_ptr() as *mut RtSysQueueItemHeader };
                    }
                    self.inner_queue.free = unsafe { Some(NonNull::new_unchecked(hdr as *mut u8)) };
                }
            } else {
                self.inner_queue.read_pos = 0;
                self.inner_queue.write_pos = 0;
            }
        }

        self.inner_queue.item_in_queue = 0;

        drop(spin_guard);

        Cpu::get_current_scheduler().do_task_schedule();

        Ok(())
    }

    #[cfg(feature = "messagequeue_priority")]
    fn send_wait_prio(
        &mut self,
        buffer: *const u8,
        size: usize,
        prio: i32,
        timeout: i32,
        suspend_flag: SuspendFlag,
    ) -> Result<usize, Error> {
        self.send_wait_internal(buffer, size, prio, timeout, suspend_flag)
    }

    #[cfg(feature = "messagequeue_priority")]
    fn receive_prio(
        &mut self,
        buffer: *mut u8,
        size: usize,
        prio: *mut i32,
        timeout: i32,
        suspend_flag: SuspendFlag,
    ) -> Result<usize, Error> {
        self.receive_internal(buffer, size, prio, timeout, suspend_flag)
    }
}

/// bindgen for MessageQueue
#[allow(improper_ctypes_definitions)]
#[no_mangle]
pub extern "C" fn bindgen_mq(_mq: MessageQueue) {
    0;
}
