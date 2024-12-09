use crate::{
    allocator::{align_up_size, rt_free, rt_malloc},
    clock::rt_tick_get,
    cpu::Cpu,
    error::{code, Error},
    impl_kobject,
    klibc::rt_memcpy,
    list_head_for_each,
    object::*,
    print, println,
    rt_bindings::{
        rt_debug_not_in_interrupt, rt_debug_scheduler_available, rt_err_t, rt_int32_t, rt_object,
        rt_object_hook_call, rt_size_t, rt_ssize_t, rt_uint16_t, rt_uint32_t, rt_uint8_t,
        RT_ALIGN_SIZE, RT_EFULL, RT_EINTR, RT_EINVAL, RT_ENOMEM, RT_EOK, RT_ERROR, RT_ETIMEOUT,
        RT_INTERRUPTIBLE, RT_IPC_CMD_RESET, RT_IPC_FLAG_FIFO, RT_IPC_FLAG_PRIO, RT_KILLABLE,
        RT_MQ_ENTRY_MAX, RT_TIMER_CTRL_SET_TIME, RT_UNINTERRUPTIBLE,
    },
    sync::ipc_common::*,
    thread::RtThread,
};
use blue_infra::list::doubly_linked_list::ListHead;
use core::ptr::NonNull;
#[allow(unused_imports)]
use core::{
    alloc::AllocError,
    cell::UnsafeCell,
    ffi::{self, c_char, c_void},
    marker::PhantomPinned,
    mem,
    mem::MaybeUninit,
    ptr::null_mut,
    slice,
};

use crate::alloc::boxed::Box;
use core::pin::Pin;
use kernel::{fmt, str::CString};

use cfg_if;
use pinned_init::*;

#[pin_data(PinnedDrop)]
pub struct KMessageQueue {
    #[pin]
    raw: UnsafeCell<RtMessageQueue>,
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
        fn init_raw(msg_size: usize, max_msgs: usize) -> impl PinInit<UnsafeCell<RtMessageQueue>> {
            let init = move |slot: *mut UnsafeCell<RtMessageQueue>| {
                let slot: *mut RtMessageQueue = slot.cast();
                unsafe {
                    let cur_ref = &mut *slot;

                    if let Ok(s) = CString::try_from_fmt(fmt!("{:p}", slot)) {
                        cur_ref.init_new_storage(
                            s.as_ptr() as *const i8,
                            msg_size as u16,
                            max_msgs as u16,
                            RT_IPC_FLAG_PRIO as u8,
                        );
                    } else {
                        let default = "default";
                        cur_ref.init_new_storage(
                            default.as_ptr() as *const i8,
                            msg_size as u16,
                            max_msgs as u16,
                            RT_IPC_FLAG_PRIO as u8,
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

        let result = unsafe { (*self.raw.get()).send(msg_ptr, size) };
        if result == RT_EOK as rt_err_t {
            Ok(())
        } else {
            Err(Error::from_errno(result))
        }
    }

    pub fn receive(&self, timeout: i32) -> Result<Box<[u8]>, Error> {
        let mut buffer = null_mut();
        let mut size = 0 as usize;
        let result = unsafe { (*self.raw.get()).receive(buffer, size, timeout) };
        if result == RT_EOK as i32 {
            if buffer.is_null() || size == 0 {
                return Ok(Box::new([]));
            }
            unsafe {
                let slice_ptr = slice::from_raw_parts_mut(buffer, size);
                Ok(Box::from_raw(slice_ptr))
            }
        } else {
            Err(Error::from_errno(result))
        }
    }
}

/// MessageQueue raw structure
#[repr(C)]
#[pin_data]
pub struct RtMessageQueue {
    /// Inherit from KObjectBase
    #[pin]
    pub(crate) parent: KObjectBase,
    /// SysQueue for mailbox
    #[pin]
    pub(crate) inner_queue: RtSysQueue,
}

impl_kobject!(RtMessageQueue);

impl RtMessageQueue {
    pub fn new(
        name: [i8; NAME_MAX],
        msg_size: u16,
        max_msgs: u16,
        working_mode: u8,
        waiting_mode: u8,
    ) -> Result<Pin<Box<Self>>, AllocError> {
        assert!(
            (waiting_mode == RT_IPC_FLAG_FIFO as u8) || (waiting_mode == RT_IPC_FLAG_PRIO as u8)
        );

        rt_debug_not_in_interrupt!();

        Box::pin_init(pin_init!(Self {
            parent<-KObjectBase::new(ObjectClassType::ObjectClassMessageQueue as u8, name),
            inner_queue<-RtSysQueue::new(msg_size as usize, max_msgs as usize, working_mode as u32,
                waiting_mode as u32),
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
        waiting_mode: u8,
    ) -> i32 {
        assert!(
            (waiting_mode == RT_IPC_FLAG_FIFO as u8) || (waiting_mode == RT_IPC_FLAG_PRIO as u8)
        );
        self.parent
            .init(ObjectClassType::ObjectClassMessageQueue as u8, name);

        if buffer.is_null() || item_size == 0 || buffer_size == 0 {
            return -(RT_EINVAL as i32);
        }

        let mut max_count = 1;
        if working_mode == IPC_SYS_QUEUE_FIFO as u8 {
            max_count = (buffer_size / item_size)
        } else if working_mode == IPC_SYS_QUEUE_PRIO as u8 {
            let item_align_size = align_up_size(item_size as usize, RT_ALIGN_SIZE as usize);
            max_count =
                buffer_size as usize / (item_align_size + mem::size_of::<RtSysQueueItemHeader>());
        } else {
            return -(RT_EINVAL as i32);
        }

        if max_count == 0 {
            return -(RT_EINVAL as i32);
        }

        self.inner_queue
            .init(
                buffer,
                item_size,
                max_count,
                working_mode as u32,
                waiting_mode as u32,
            )
            .to_errno()
    }

    pub fn init_new_storage(
        &mut self,
        name: *const i8,
        msg_size: u16,
        max_msgs: u16,
        flag: u8,
    ) -> i32 {
        RT_EOK as i32
    }

    #[inline]
    pub fn detach(&mut self) {
        assert_eq!(
            self.type_name(),
            ObjectClassType::ObjectClassMessageQueue as u8
        );

        self.inner_queue.dequeue_waiter.inner_locked_wake_all();
        self.inner_queue.enqueue_waiter.inner_locked_wake_all();

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
        waiting_mode: u8,
    ) -> *mut RtMessageQueue {
        let message_queue = RtMessageQueue::new(
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

        rt_debug_not_in_interrupt!();

        self.inner_queue.dequeue_waiter.inner_locked_wake_all();
        self.inner_queue.enqueue_waiter.inner_locked_wake_all();

        self.parent.delete();
    }

    fn send_wait_internal(
        &mut self,
        buffer: *const u8,
        size: usize,
        prio: i32,
        timeout: i32,
        suspend_flag: u32,
    ) -> i32 {
        let mut timeout = timeout;

        assert_eq!(
            self.type_name(),
            ObjectClassType::ObjectClassMessageQueue as u8
        );

        assert!(size != 0);

        #[allow(unused_variables)]
        let scheduler = timeout != 0;
        rt_debug_scheduler_available!(scheduler);

        if size > self.inner_queue.item_size {
            return -(RT_ERROR as i32);
        }

        let mut tick_delta = 0;
        let thread_ptr = crate::current_thread_ptr!();

        if thread_ptr.is_null() {
            return -(RT_ERROR as i32);
        }

        // SAFETY: thread_ptr is null checked
        let thread = unsafe { &mut *thread_ptr };

        unsafe {
            rt_object_hook_call!(
                rt_object_put_hook,
                &mut self.parent as *mut KObjectBase as *mut rt_object
            );
        }

        self.inner_queue.lock();

        if self.inner_queue.is_full() && timeout == 0 {
            self.inner_queue.unlock();
            return -(RT_EFULL as i32);
        }

        while self.inner_queue.is_full() {
            thread.error = code::EINTR;

            if timeout == 0 {
                self.inner_queue.unlock();
                return -(RT_EFULL as i32);
            }

            let ret = self
                .inner_queue
                .enqueue_waiter
                .wait(thread, suspend_flag as u32);

            if ret != RT_EOK as i32 {
                self.inner_queue.unlock();
                return ret;
            }

            if timeout > 0 {
                tick_delta = rt_tick_get();
                thread.thread_timer.timer_control(
                    RT_TIMER_CTRL_SET_TIME as u32,
                    (&mut timeout) as *mut i32 as *mut c_void,
                );
                thread.thread_timer.start();
            }

            self.inner_queue.unlock();

            Cpu::get_current_scheduler().do_task_schedule();

            if thread.error != code::EOK {
                return thread.error.to_errno();
            }

            self.inner_queue.lock();

            if timeout > 0 {
                tick_delta = rt_tick_get() - tick_delta;
                timeout -= tick_delta as i32;
                if timeout < 0 {
                    timeout = 0;
                }
            }
        }

        cfg_if::cfg_if! {
            if #[cfg(feature = "RT_USING_MESSAGEQUEUE_PRIORITY")] {
                self.inner_queue.push_prio(buffer, size, prio);
            } else {
                self.inner_queue.push_fifo(buffer, size);
            }
        }

        if !self.inner_queue.dequeue_waiter.is_empty() {
            self.inner_queue.dequeue_waiter.wake();
            self.inner_queue.unlock();

            Cpu::get_current_scheduler().do_task_schedule();

            return RT_EOK as i32;
        }

        self.inner_queue.unlock();

        RT_EOK as i32
    }

    pub fn send(&mut self, buffer: *const u8, size: usize) -> i32 {
        self.send_wait(buffer, size, 0)
    }

    fn send_interruptible(&mut self, buffer: *const u8, size: usize) -> i32 {
        self.send_wait_interruptible(buffer, size, 0)
    }

    fn send_killable(&mut self, buffer: *const u8, size: usize) -> i32 {
        self.send_wait_killable(buffer, size, 0)
    }

    pub fn send_wait(&mut self, buffer: *const u8, size: usize, timeout: i32) -> i32 {
        self.send_wait_internal(buffer, size, 0, timeout, RT_UNINTERRUPTIBLE as u32)
    }

    pub fn send_wait_interruptible(&mut self, buffer: *const u8, size: usize, timeout: i32) -> i32 {
        self.send_wait_internal(buffer, size, 0, timeout, RT_INTERRUPTIBLE as u32)
    }

    pub fn send_wait_killable(&mut self, buffer: *const u8, size: usize, timeout: i32) -> i32 {
        self.send_wait_internal(buffer, size, 0, timeout, RT_KILLABLE as u32)
    }

    pub fn urgent(&mut self, buffer: *const u8, size: usize) -> i32 {
        assert_eq!(
            self.type_name(),
            ObjectClassType::ObjectClassMessageQueue as u8
        );
        assert!(!buffer.is_null());
        assert!(size != 0);

        if size > self.inner_queue.item_size {
            return -(RT_ERROR as i32);
        }

        unsafe {
            rt_object_hook_call!(
                rt_object_put_hook,
                &mut self.parent as *mut KObjectBase as *mut rt_object
            );
        }

        let mut urgent_size = 0;
        cfg_if::cfg_if! {
            if #[cfg(feature = "RT_USING_MESSAGEQUEUE_PRIORITY")] {
                urgent_size = self.inner_queue.urgent_prio(buffer, size, prio);
            } else {
                urgent_size = self.inner_queue.urgent_fifo(buffer, size);
            }
        }

        if urgent_size > 0 {
            RT_EOK as i32
        } else {
            -(RT_ERROR as i32)
        }
    }

    fn receive_internal(
        &mut self,
        buffer: *mut u8,
        size: usize,
        prio: *mut i32,
        timeout: i32,
        suspend_flag: u32,
    ) -> i32 {
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

        let thread_ptr = crate::current_thread_ptr!();

        if thread_ptr.is_null() {
            return -(RT_ERROR as i32);
        }

        // SAFETY: thread_ptr is null checked
        let thread = unsafe { &mut *thread_ptr };

        unsafe {
            rt_object_hook_call!(
                rt_object_trytake_hook,
                &mut self.parent as *mut KObjectBase as *mut rt_object
            );
        }

        self.inner_queue.lock();

        if self.inner_queue.is_empty() && timeout == 0 {
            self.inner_queue.unlock();
            return -(RT_ETIMEOUT as i32);
        }

        while self.inner_queue.is_empty() {
            thread.error = code::EINTR;

            if timeout == 0 {
                self.inner_queue.unlock();
                thread.error = code::ETIMEOUT;
                return thread.error.to_errno();
            }

            let ret = self
                .inner_queue
                .dequeue_waiter
                .wait(thread, suspend_flag as u32);
            if ret != RT_EOK as i32 {
                self.inner_queue.unlock();
                return ret;
            }

            let mut tick_delta: rt_uint32_t = 0;
            if timeout > 0 {
                tick_delta = rt_tick_get();

                thread.thread_timer.timer_control(
                    RT_TIMER_CTRL_SET_TIME as u32,
                    (&mut timeout) as *mut i32 as *mut c_void,
                );
                (*thread).thread_timer.start();
            }

            self.inner_queue.unlock();
            Cpu::get_current_scheduler().do_task_schedule();

            if thread.error != code::EOK {
                return thread.error.to_errno();
            }

            self.inner_queue.lock();

            if timeout > 0 {
                tick_delta = rt_tick_get() - tick_delta;
                timeout -= tick_delta as rt_int32_t;
                if timeout < 0 {
                    timeout = 0;
                }
            }
        }

        cfg_if::cfg_if! {
            if #[cfg(feature = "RT_USING_MESSAGEQUEUE_PRIORITY")] {
                self.inner_queue.pop_prio(buffer, size, prio);
            } else {
                let mut buffer_mut = buffer;
                self.inner_queue.pop_fifo(&mut buffer_mut, size);
            }
        }

        let mut need_schedule = false;
        if !self.inner_queue.enqueue_waiter.is_empty() {
            self.inner_queue.enqueue_waiter.wake();
            need_schedule = true;
        }

        self.inner_queue.unlock();

        unsafe {
            rt_object_hook_call!(
                rt_object_take_hook,
                &mut self.parent as *mut KObjectBase as *mut rt_object
            );
        }

        if need_schedule {
            Cpu::get_current_scheduler().do_task_schedule();
        }

        size as i32
    }

    pub fn receive(&mut self, buffer: *mut u8, size: usize, timeout: i32) -> i32 {
        self.receive_internal(buffer, size, null_mut(), timeout, RT_UNINTERRUPTIBLE as u32)
    }

    pub fn receive_interruptible(&mut self, buffer: *mut u8, size: usize, timeout: i32) -> i32 {
        self.receive_internal(buffer, size, null_mut(), timeout, RT_INTERRUPTIBLE as u32)
    }

    pub fn receive_killable(&mut self, buffer: *mut u8, size: usize, timeout: i32) -> i32 {
        self.receive_internal(buffer, size, null_mut(), timeout, RT_KILLABLE as u32)
    }

    pub fn control(&mut self, cmd: i32, _arg: *mut u8) -> i32 {
        assert_eq!(
            self.type_name(),
            ObjectClassType::ObjectClassMessageQueue as u8
        );

        if cmd == RT_IPC_CMD_RESET as i32 {
            self.inner_queue.lock();

            self.inner_queue.dequeue_waiter.inner_locked_wake_all();
            self.inner_queue.enqueue_waiter.inner_locked_wake_all();

            cfg_if::cfg_if! {
                if #[cfg(feature = "RT_USING_MESSAGEQUEUE_PRIORITY")] {
                    while !self.inner_queue.head.is_none() {
                        let hdr = self.inner_queue.head.unwrap().as_ptr() as *mut RtSysQueueItemHeader;

                        self.inner_queue.head =
                            unsafe { Some(NonNull::new_unchecked((*hdr).next as *mut u8)) };
                        if self.inner_queue.tail.unwrap().as_ptr() == hdr as *mut u8 {
                            self.inner_queue.tail = None;
                        }

                        unsafe { (*hdr).next =
                            self.inner_queue.free.unwrap().as_ptr() as *mut RtSysQueueItemHeader };
                        self.inner_queue.free = unsafe { Some(NonNull::new_unchecked(hdr as *mut u8)) };
                    }
                } else {
                    self.inner_queue.read_pos = 0;
                    self.inner_queue.write_pos = 0;
                }
            }

            self.inner_queue.item_in_queue = 0;
            self.inner_queue.unlock();

            Cpu::get_current_scheduler().do_task_schedule();

            return RT_EOK as i32;
        }

        -(RT_ERROR as i32)
    }

    #[cfg(feature = "RT_USING_MESSAGEQUEUE_PRIORITY")]
    fn send_wait_prio(
        &mut self,
        buffer: *const u8,
        size: usize,
        prio: i32,
        timeout: i32,
        suspend_flag: u32,
    ) -> rt_err_t {
        self.send_wait_internal(buffer, size, prio, timeout, suspend_flag)
    }

    #[cfg(feature = "RT_USING_MESSAGEQUEUE_PRIORITY")]
    fn receive_prio(
        &mut self,
        buffer: *mut u8,
        size: usize,
        prio: *mut i32,
        timeout: i32,
        suspend_flag: u32,
    ) -> rt_ssize_t {
        self.receive_internal(buffer, size, prio, timeout, suspend_flag)
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
    assert!((flag == RT_IPC_FLAG_FIFO as u8) || (flag == RT_IPC_FLAG_PRIO as u8));
    let mut queue_working_mode = IPC_SYS_QUEUE_FIFO as u8;
    #[cfg(feature = "RT_USING_MESSAGEQUEUE_PRIORITY")]
    {
        queue_working_mode = IPC_SYS_QUEUE_PRIO as u8;
    }
    (*mq).init(
        name,
        msgpool as *mut u8,
        msg_size as usize,
        pool_size as usize,
        queue_working_mode as u8,
        flag,
    )
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
    let mut queue_working_mode = IPC_SYS_QUEUE_FIFO as u8;
    #[cfg(feature = "RT_USING_MESSAGEQUEUE_PRIORITY")]
    {
        queue_working_mode = IPC_SYS_QUEUE_PRIO as u8;
    }
    RtMessageQueue::new_raw(
        name,
        msg_size as usize,
        max_msgs as usize,
        queue_working_mode,
        flag,
    )
}

#[cfg(all(feature = "RT_USING_MESSAGEQUEUE", feature = "RT_USING_HEAP"))]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_delete(mq: *mut RtMessageQueue) -> rt_err_t {
    assert!(mq != null_mut());

    (*mq).delete_raw();

    RT_EOK as rt_err_t
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_entry(mq: *mut RtMessageQueue) -> rt_uint16_t {
    assert!(!mq.is_null());
    (*mq).inner_queue.count() as rt_uint16_t
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_send(
    mq: *mut RtMessageQueue,
    buffer: *const core::ffi::c_void,
    size: rt_size_t,
) -> rt_err_t {
    assert!(!mq.is_null());
    (*mq).send(buffer as *const u8, size as usize)
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_send_interruptible(
    mq: *mut RtMessageQueue,
    buffer: *const core::ffi::c_void,
    size: rt_size_t,
) -> rt_err_t {
    assert!(!mq.is_null());
    (*mq).send_interruptible(buffer as *const u8, size as usize)
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_send_killable(
    mq: *mut RtMessageQueue,
    buffer: *const core::ffi::c_void,
    size: rt_size_t,
) -> rt_err_t {
    assert!(!mq.is_null());
    (*mq).send_killable(buffer as *const u8, size as usize)
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
    (*mq).send_wait(buffer as *const u8, size as usize, timeout)
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
    (*mq).send_wait_interruptible(buffer as *const u8, size as usize, timeout)
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
    (*mq).send_wait_killable(buffer as *const u8, size as usize, timeout)
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_urgent(
    mq: *mut RtMessageQueue,
    buffer: *const core::ffi::c_void,
    size: rt_size_t,
) -> rt_err_t {
    assert!(!mq.is_null());
    (*mq).urgent(buffer as *const u8, size as usize) as rt_err_t
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
    (*mq).receive(buffer as *mut u8, size as usize, timeout)
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
    (*mq).receive_interruptible(buffer as *mut u8, size as usize, timeout)
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
    (*mq).receive_killable(buffer as *mut u8, size as usize, timeout)
}

#[cfg(feature = "RT_USING_MESSAGEQUEUE")]
#[no_mangle]
pub unsafe extern "C" fn rt_mq_control(
    mq: *mut RtMessageQueue,
    cmd: core::ffi::c_int,
    _arg: *mut core::ffi::c_void,
) -> rt_err_t {
    assert!(!mq.is_null());
    (*mq).control(cmd, _arg as *mut u8)
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
    (*mq).send_wait_prio(
        buffer as *const u8,
        size as usize,
        prio,
        timeout,
        suspend_flag as u32,
    )
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
    (*mq).receive_prio(
        buffer as *mut u8,
        size as usize,
        prio,
        timeout,
        suspend_flag as u32,
    )
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
        let _ = crate::format_name!(msgqueue.parent.name.as_ptr(), 8);
        print!(" {:04} ", msgqueue.inner_queue.count());
        if msgqueue.inner_queue.dequeue_waiter.is_empty() {
            println!(" {}", msgqueue.inner_queue.dequeue_waiter.count());
        } else {
            print!(" {}:", msgqueue.inner_queue.dequeue_waiter.count());
            let head = &msgqueue.inner_queue.dequeue_waiter.working_queue;
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
        ObjectClassType::ObjectClassMessageQueue as u8,
    );
}
