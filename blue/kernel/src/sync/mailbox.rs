use crate::{
    allocator::{rt_free, rt_malloc},
    clock::rt_tick_get,
    cpu::Cpu,
    error::{code, Error},
    impl_kobject, list_head_for_each,
    object::*,
    print, println,
    rt_bindings::{
        rt_debug_not_in_interrupt, rt_debug_scheduler_available, rt_err_t, rt_int32_t, rt_object,
        rt_object_hook_call, rt_size_t, rt_ubase_t, rt_uint32_t, rt_uint8_t, RT_EFULL, RT_EINTR,
        RT_EINVAL, RT_ENOMEM, RT_EOK, RT_ERROR, RT_ETIMEOUT, RT_EVENT_FLAG_AND, RT_INTERRUPTIBLE,
        RT_IPC_CMD_RESET, RT_IPC_FLAG_FIFO, RT_IPC_FLAG_PRIO, RT_KILLABLE, RT_MB_ENTRY_MAX,
        RT_TIMER_CTRL_SET_TIME, RT_UNINTERRUPTIBLE,
    },
    sync::{ipc_common::*, lock::spinlock::RawSpin},
    thread::RtThread,
};
use blue_infra::list::doubly_linked_list::ListHead;
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

use crate::alloc::boxed::Box;
use core::pin::Pin;
use kernel::{fmt, str::CString};

use pinned_init::*;

#[pin_data(PinnedDrop)]
pub struct KMailbox {
    #[pin]
    raw: UnsafeCell<RtMailbox>,
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
        fn init_raw(size: usize) -> impl PinInit<UnsafeCell<RtMailbox>> {
            let init = move |slot: *mut UnsafeCell<RtMailbox>| {
                let slot: *mut RtMailbox = slot.cast();
                unsafe {
                    let cur_ref = &mut *slot;

                    if let Ok(s) = CString::try_from_fmt(fmt!("{:p}", slot)) {
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
                        RT_IPC_FLAG_FIFO as u32,
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
        let result = unsafe { (*self.raw.get()).send(set) };
        if result == RT_EOK as rt_err_t {
            Ok(())
        } else {
            Err(Error::from_errno(result))
        }
    }

    pub fn receive(&self, timeout: i32) -> Result<usize, Error> {
        let mut retmsg = 0 as usize;
        let result = unsafe { (*self.raw.get()).receive(&mut retmsg, timeout) };
        if result == RT_EOK as rt_err_t {
            Ok(retmsg)
        } else {
            Err(Error::from_errno(result))
        }
    }
}

/// Mailbox raw structure
#[repr(C)]
#[pin_data(PinnedDrop)]
pub struct RtMailbox {
    /// Inherit from IPCObject
    #[pin]
    pub(crate) parent: KObjectBase,
    /// Spin lock semaphore used
    pub(crate) spinlock: RawSpin,
    #[pin]
    /// SysQueue for semaphore
    #[pin]
    pub(crate) inner_queue: RtSysQueue,
}

impl_kobject!(RtMailbox);

#[pinned_drop]
impl PinnedDrop for RtMailbox {
    fn drop(self: Pin<&mut Self>) {}
}

impl RtMailbox {
    #[inline]
    pub fn new(name: [i8; NAME_MAX], size: usize, flag: u8) -> impl PinInit<Self> {
        assert!((flag == RT_IPC_FLAG_FIFO as u8) || (flag == RT_IPC_FLAG_PRIO as u8));

        rt_debug_not_in_interrupt!();

        pin_init!(Self {
            parent<-KObjectBase::new(ObjectClassType::ObjectClassMailBox as u8, name),
            spinlock:RawSpin::new(),
            inner_queue<-RtSysQueue::new(mem::size_of::<usize>(), size, 0, flag as u32),
        })
    }
    #[inline]
    pub fn init(&mut self, name: *const i8, msg_pool: *mut u8, size: usize, flag: u8) {
        assert!((flag == RT_IPC_FLAG_FIFO as u8) || (flag == RT_IPC_FLAG_PRIO as u8));

        self.parent
            .init(ObjectClassType::ObjectClassMailBox as u8, name);

        self.inner_queue
            .init(null_mut(), mem::size_of::<usize>(), size, 0, flag as u32);
    }

    #[inline]
    pub fn detach(&mut self) {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassMailBox as u8);

        self.inner_queue.receiver.inner_locked_wake_all();
        self.inner_queue.sender.inner_locked_wake_all();

        if self.is_static_kobject() {
            self.parent.detach();
        }
    }

    #[inline]
    pub fn new_raw(name: *const i8, size: usize, flag: u8) -> *mut Self {
        let mailbox = Box::pin_init(RtMailbox::new(char_ptr_to_array(name), size, flag));
        match mailbox {
            Ok(mb) => unsafe { Box::leak(Pin::into_inner_unchecked(mb)) },
            Err(_) => return null_mut(),
        }
    }

    #[inline]
    pub fn delete_raw(&mut self) {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassMailBox as u8);
        assert!(!self.is_static_kobject());

        rt_debug_not_in_interrupt!();

        self.inner_queue.receiver.inner_locked_wake_all();
        self.inner_queue.sender.inner_locked_wake_all();
        self.inner_queue.free_storage_internal();

        self.parent.delete();
    }

    fn send_wait_internal(&mut self, value: usize, timeout: i32, suspend_flag: u32) -> i32 {
        let mut timeout = timeout;
        #[allow(unused_variables)]
        let scheduler = timeout != 0;
        rt_debug_scheduler_available!(scheduler);

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

        self.spinlock.lock();

        if self.inner_queue.is_full() && timeout == 0 {
            self.spinlock.unlock();
            return -(RT_EFULL as i32);
        }

        while self.inner_queue.is_full() {
            thread.error = code::EINTR;

            if timeout == 0 {
                self.spinlock.unlock();

                return -(RT_EFULL as i32);
            }

            let ret = self.inner_queue.sender.wait(thread, suspend_flag);

            if ret != RT_EOK as i32 {
                self.spinlock.unlock();
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

            self.spinlock.unlock();

            Cpu::get_current_scheduler().do_task_schedule();

            if thread.error != code::EOK {
                return thread.error.to_errno();
            }

            self.spinlock.lock();

            if timeout > 0 {
                tick_delta = rt_tick_get() - tick_delta;
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
            self.spinlock.unlock();
            return -(RT_EFULL as i32);
        }

        if !self.inner_queue.receiver.is_empty() {
            self.inner_queue.receiver.wake();

            self.spinlock.unlock();

            Cpu::get_current_scheduler().do_task_schedule();

            return RT_EOK as i32;
        }

        self.spinlock.unlock();

        return RT_EOK as i32;
    }

    pub fn send_wait(&mut self, value: usize, timeout: i32) -> i32 {
        self.send_wait_internal(value, timeout, RT_UNINTERRUPTIBLE as u32)
    }

    pub fn send_wait_interruptible(&mut self, value: usize, timeout: i32) -> i32 {
        self.send_wait_internal(value, timeout, RT_INTERRUPTIBLE as u32)
    }

    pub fn send_wait_killable(&mut self, value: usize, timeout: i32) -> i32 {
        self.send_wait_internal(value, timeout, RT_KILLABLE as u32)
    }

    pub fn send(&mut self, value: usize) -> i32 {
        self.send_wait(value, 0)
    }

    pub fn send_interruptible(&mut self, value: usize) -> i32 {
        self.send_wait_interruptible(value, 0)
    }

    pub fn send_killable(&mut self, value: usize) -> i32 {
        self.send_wait_killable(value, 0)
    }

    pub fn urgent(&mut self, value: usize) -> i32 {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassMailBox as u8);

        // SAFETY： hook and memory operation should be ensured safe
        unsafe {
            rt_object_hook_call!(
                rt_object_put_hook,
                &mut self.parent as *mut KObjectBase as *mut rt_object
            );
        }

        self.spinlock.lock();

        if self.inner_queue.is_full() {
            self.spinlock.unlock();
            return -(RT_EFULL as i32);
        }

        self.inner_queue
            .urgent_fifo(&value as *const usize as *const u8, mem::size_of::<usize>());

        if !self.inner_queue.receiver.is_empty() {
            self.inner_queue.receiver.wake();

            self.spinlock.unlock();

            Cpu::get_current_scheduler().do_task_schedule();

            return RT_EOK as i32;
        }

        self.spinlock.unlock();

        RT_EOK as i32
    }

    fn receive_internal(&mut self, value: &mut usize, timeout: i32, suspend_flag: u32) -> i32 {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassMailBox as u8);

        let mut timeout = timeout;
        #[allow(unused_variables)]
        let scheduler = timeout != 0;
        rt_debug_scheduler_available!(scheduler);

        let mut tick_delta = 0;

        let thread_ptr = unsafe { crate::current_thread!().unwrap().as_mut() };
        // SAFETY： hook and memory operation should be ensured safe
        unsafe {
            rt_object_hook_call!(
                rt_object_trytake_hook,
                &mut self.parent as *mut KObjectBase as *mut rt_object
            );
        }

        self.spinlock.lock();

        if self.inner_queue.is_empty() && timeout == 0 {
            self.spinlock.unlock();
            return -(RT_ETIMEOUT as i32);
        }

        let thread = unsafe { &mut *thread_ptr };
        while self.inner_queue.is_empty() {
            thread.error = code::EINTR;

            if timeout == 0 {
                self.spinlock.unlock();
                thread.error = code::ETIMEOUT;

                return -(RT_ETIMEOUT as i32);
            }

            let ret = self.inner_queue.receiver.wait(thread, suspend_flag as u32);
            if ret != RT_EOK as i32 {
                self.spinlock.unlock();
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

            self.spinlock.unlock();

            Cpu::get_current_scheduler().do_task_schedule();

            if thread.error != code::EOK {
                return thread.error.to_errno();
            }

            self.spinlock.lock();

            if timeout > 0 {
                tick_delta = rt_tick_get() - tick_delta;
                timeout -= tick_delta as i32;
                if timeout < 0 {
                    timeout = 0;
                }
            }
        }

        self.inner_queue.pop_fifo(
            &mut (value as *mut usize as *mut u8),
            mem::size_of::<usize>(),
        );

        if !self.inner_queue.sender.is_empty() {
            if let Some(node) = self.inner_queue.sender.head() {
                let thread: *mut RtThread =
                    unsafe { crate::thread_list_node_entry!(node.as_ptr()) };
                unsafe {
                    (*thread).error = code::EOK;
                    (*thread).resume();
                }
            }

            self.spinlock.unlock();

            unsafe {
                rt_object_hook_call!(
                    rt_object_take_hook,
                    &mut self.parent as *mut KObjectBase as *mut rt_object
                );
            }

            Cpu::get_current_scheduler().do_task_schedule();

            return RT_EOK as i32;
        }

        self.spinlock.unlock();

        unsafe {
            rt_object_hook_call!(
                rt_object_take_hook,
                &mut self.parent as *mut KObjectBase as *mut rt_object
            );
        }

        RT_EOK as i32
    }

    pub fn receive(&mut self, value: &mut usize, timeout: i32) -> i32 {
        self.receive_internal(value, timeout, RT_UNINTERRUPTIBLE)
    }

    pub fn receive_interruptible(&mut self, value: &mut usize, timeout: i32) -> i32 {
        self.receive_internal(value, timeout, RT_INTERRUPTIBLE)
    }

    pub fn receive_killable(&mut self, value: &mut usize, timeout: i32) -> i32 {
        self.receive_internal(value, timeout, RT_KILLABLE)
    }

    pub fn control(&mut self, cmd: i32, _arg: *mut core::ffi::c_void) -> i32 {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassMailBox as u8);

        if cmd == RT_IPC_CMD_RESET as i32 {
            self.spinlock.lock();

            self.inner_queue.receiver.inner_locked_wake_all();
            self.inner_queue.sender.inner_locked_wake_all();

            self.inner_queue.item_in_queue = 0;
            self.inner_queue.read_pos = 0;
            self.inner_queue.write_pos = 0;

            self.spinlock.unlock();

            Cpu::get_current_scheduler().do_task_schedule();

            return RT_EOK as i32;
        }

        RT_EOK as i32
    }
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_init(
    mb: *mut RtMailbox,
    name: *const core::ffi::c_char,
    msgpool: *mut core::ffi::c_void,
    size: rt_size_t,
    flag: rt_uint8_t,
) -> rt_err_t {
    assert!(!mb.is_null());

    (*mb).init(name, msgpool as *mut u8, size as usize, flag);

    RT_EOK as rt_err_t
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_detach(mb: *mut RtMailbox) -> rt_err_t {
    assert!(!mb.is_null());

    (*mb).detach();

    return RT_EOK as rt_err_t;
}

#[cfg(all(feature = "RT_USING_MAILBOX", feature = "RT_USING_HEAP"))]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_create(
    name: *const core::ffi::c_char,
    size: rt_size_t,
    flag: rt_uint8_t,
) -> *mut RtMailbox {
    RtMailbox::new_raw(name, size as usize, flag)
}

#[cfg(all(feature = "RT_USING_MAILBOX", feature = "RT_USING_HEAP"))]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_delete(mb: *mut RtMailbox) -> rt_err_t {
    assert!(!mb.is_null());

    (*mb).delete_raw();

    RT_EOK as rt_err_t
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_send_wait(
    mb: *mut RtMailbox,
    value: rt_ubase_t,
    timeout: rt_int32_t,
) -> rt_err_t {
    assert!(!mb.is_null());
    (*mb).send_wait(value as usize, timeout) as rt_err_t
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_send_wait_interruptible(
    mb: *mut RtMailbox,
    value: rt_ubase_t,
    timeout: rt_int32_t,
) -> rt_err_t {
    assert!(!mb.is_null());
    (*mb).send_wait_interruptible(value as usize, timeout) as rt_err_t
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_send_wait_killable(
    mb: *mut RtMailbox,
    value: rt_ubase_t,
    timeout: rt_int32_t,
) -> rt_err_t {
    assert!(!mb.is_null());
    (*mb).send_wait_killable(value as usize, timeout) as rt_err_t
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_send(mb: *mut RtMailbox, value: ffi::c_ulong) -> rt_err_t {
    assert!(!mb.is_null());
    (*mb).send(value as usize) as rt_err_t
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_send_interruptible(
    mb: *mut RtMailbox,
    value: ffi::c_ulong,
) -> rt_err_t {
    assert!(!mb.is_null());
    (*mb).send_interruptible(value as usize) as rt_err_t
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_send_killable(mb: *mut RtMailbox, value: ffi::c_ulong) -> rt_err_t {
    assert!(!mb.is_null());
    (*mb).send_killable(value as usize) as rt_err_t
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_urgent(mb: *mut RtMailbox, value: ffi::c_ulong) -> rt_err_t {
    assert!(!mb.is_null());
    (*mb).urgent(value as usize) as rt_err_t
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_recv(
    mb: *mut RtMailbox,
    value: *mut ffi::c_ulong,
    timeout: rt_int32_t,
) -> rt_err_t {
    assert!(!mb.is_null());
    let mut receive_val = 0usize;
    let ret_val = (*mb).receive(&mut receive_val, timeout) as rt_err_t;
    *value = receive_val as ffi::c_ulong;
    ret_val
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_recv_interruptible(
    mb: *mut RtMailbox,
    value: *mut ffi::c_ulong,
    timeout: rt_int32_t,
) -> rt_err_t {
    assert!(!mb.is_null());
    let mut receive_val = 0usize;
    let ret_val = (*mb).receive_interruptible(&mut receive_val, timeout) as rt_err_t;
    *value = receive_val as ffi::c_ulong;
    ret_val
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_recv_killable(
    mb: *mut RtMailbox,
    value: *mut ffi::c_ulong,
    timeout: rt_int32_t,
) -> rt_err_t {
    assert!(!mb.is_null());
    let mut receive_val = 0usize;
    let ret_val = (*mb).receive_killable(&mut receive_val, timeout) as rt_err_t;
    *value = receive_val as ffi::c_ulong;
    ret_val
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_control(
    mb: *mut RtMailbox,
    cmd: core::ffi::c_int,
    _arg: *mut core::ffi::c_void,
) -> rt_err_t {
    assert!(!mb.is_null());
    (*mb).control(cmd, _arg) as rt_err_t
}

#[no_mangle]
#[allow(unused_unsafe)]
pub extern "C" fn rt_mailbox_info() {
    let callback_forword = || {
        println!("mailbox  entry size suspend thread");
        println!("-------- ----  ---- --------------");
    };
    let callback = |node: &ListHead| unsafe {
        let mailbox =
            &*(crate::list_head_entry!(node.as_ptr(), KObjectBase, list) as *const RtMailbox);
        let _ = crate::format_name!(mailbox.parent.name.as_ptr(), 8);
        print!(" {:04} ", mailbox.inner_queue.count());
        print!(" {:04} ", mailbox.inner_queue.item_max_count);
        if mailbox.inner_queue.receiver.is_empty() {
            println!("{}", mailbox.inner_queue.receiver.count());
        } else {
            print!("{}:", mailbox.inner_queue.receiver.count());
            let head = &mailbox.inner_queue.receiver.working_queue;
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
        ObjectClassType::ObjectClassMailBox as u8,
    );
}
