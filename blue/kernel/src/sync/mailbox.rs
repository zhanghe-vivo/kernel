use crate::{
    allocator::{rt_free, rt_malloc},
    clock::rt_tick_get,
    cpu::Cpu,
    error::Error,
    impl_kobject, list_head_for_each,
    object::{ObjectClassType, NAME_MAX, *},
    print, println,
    rt_bindings::{
        rt_debug_not_in_interrupt, rt_debug_scheduler_available, rt_err_t, rt_int32_t, rt_object,
        rt_object_hook_call, rt_size_t, rt_ubase_t, rt_uint32_t, rt_uint8_t, RT_EFULL, RT_EINTR,
        RT_EINVAL, RT_ENOMEM, RT_EOK, RT_ERROR, RT_ETIMEOUT, RT_EVENT_FLAG_AND, RT_INTERRUPTIBLE,
        RT_IPC_CMD_RESET, RT_IPC_FLAG_FIFO, RT_IPC_FLAG_PRIO, RT_KILLABLE, RT_MB_ENTRY_MAX,
        RT_TIMER_CTRL_SET_TIME, RT_UNINTERRUPTIBLE,
    },
    sync::ipc_common::*,
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
                        cur_ref.init_new_storage(
                            s.as_ptr() as *const i8,
                            size,
                            RT_IPC_FLAG_PRIO as u8,
                        );
                    } else {
                        let default = "default";
                        cur_ref.init_new_storage(
                            default.as_ptr() as *const i8,
                            size,
                            RT_IPC_FLAG_PRIO as u8,
                        );
                    }
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
    pub(crate) parent: IPCObject,
    /// Message pool buffer of mailbox
    pub(crate) msg_pool: *mut u8,
    /// Message pool buffer size
    pub(crate) size: u16,
    /// Index of messages in message pool
    pub(crate) entry: u16,
    /// Input offset of the message buffer
    pub(crate) in_offset: u16,
    /// Output offset of the message buffer
    pub(crate) out_offset: u16,
    /// Sender thread suspended on this mailbox
    #[pin]
    pub(crate) suspend_sender_thread: ListHead,
}

impl_kobject!(RtMailbox);

#[pinned_drop]
impl PinnedDrop for RtMailbox {
    fn drop(self: Pin<&mut Self>) {
        let this_mb = unsafe { Pin::get_unchecked_mut(self) };

        unsafe {
            if !this_mb.msg_pool.is_null() {
                rt_free(this_mb.msg_pool as *mut ffi::c_void);
            }
        }
    }
}

impl RtMailbox {
    #[inline]
    pub fn new(name: [i8; NAME_MAX], size: usize, flag: u8) -> impl PinInit<Self> {
        assert!((flag == RT_IPC_FLAG_FIFO as u8) || (flag == RT_IPC_FLAG_PRIO as u8));

        rt_debug_not_in_interrupt!();

        pin_init!(Self {
            parent<-IPCObject::new(ObjectClassType::ObjectClassMailBox as u8, name, flag),
            msg_pool: unsafe{ rt_malloc(size * mem::size_of::<usize>()) as *mut u8 },
            size: size as u16,
            entry: 0,
            in_offset: 0,
            out_offset: 0,
            suspend_sender_thread<-ListHead::new()
        })
    }
    #[inline]
    pub fn init(&mut self, name: *const i8, msg_pool: *mut u8, size: usize, flag: u8) {
        assert!((flag == RT_IPC_FLAG_FIFO as u8) || (flag == RT_IPC_FLAG_PRIO as u8));

        self.parent
            .init(ObjectClassType::ObjectClassMailBox as u8, name, flag);

        self.msg_pool = msg_pool;
        self.size = size as u16;
        self.entry = 0;
        self.in_offset = 0;
        self.out_offset = 0;

        unsafe {
            let _ = ListHead::new().__pinned_init(&mut self.suspend_sender_thread as *mut ListHead);
        }
    }

    #[inline]
    pub fn detach(&mut self) {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassMailBox as u8);
        assert!(self.is_static_kobject());

        self.parent.wake_all();
        IPCObject::resume_all_threads(&mut self.suspend_sender_thread);
        self.parent.parent.detach();
    }

    #[inline]
    pub fn init_new_storage(&mut self, name: *const i8, size: usize, flag: u8) -> i32 {
        self.parent
            .init(ObjectClassType::ObjectClassMailBox as u8, name, flag);

        self.size = size as u16;
        // SAFETY: Ensure the alloc is successful
        unsafe {
            self.msg_pool = rt_malloc(size * mem::size_of::<usize>()) as *mut u8;
        }
        if self.msg_pool.is_null() {
            return -(RT_ENOMEM as i32);
        }

        self.entry = 0;
        self.in_offset = 0;
        self.out_offset = 0;

        unsafe {
            let _ = ListHead::new().__pinned_init(&mut self.suspend_sender_thread as *mut ListHead);
        }

        RT_EOK as i32
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

        self.parent.wake_all();
        IPCObject::resume_all_threads(&mut self.suspend_sender_thread);
        // SAFETY: null protection
        unsafe {
            if !self.msg_pool.is_null() {
                rt_free(self.msg_pool as *mut ffi::c_void);
            }
        }
        self.parent.parent.delete();
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
                &mut self.parent.parent as *mut KObjectBase as *mut rt_object
            );
        }

        self.parent.lock();

        if self.entry == self.size && timeout == 0 {
            self.parent.unlock();
            return -(RT_EFULL as i32);
        }

        while self.entry == self.size {
            thread.error = -(RT_EINTR as i32);

            if timeout == 0 {
                self.parent.unlock();

                return -(RT_EFULL as i32);
            }

            let ret = IPCObject::suspend_thread(
                &mut self.suspend_sender_thread,
                thread_ptr,
                self.parent.flag,
                suspend_flag as u32,
            );

            if ret != RT_EOK as i32 {
                self.parent.unlock();
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

            self.parent.unlock();

            Cpu::get_current_scheduler().do_task_schedule();

            if thread.error != RT_EOK as i32 {
                return thread.error;
            }

            self.parent.lock();

            if timeout > 0 {
                tick_delta = rt_tick_get() - tick_delta;
                timeout -= tick_delta as i32;
                if timeout < 0 {
                    timeout = 0;
                }
            }
        }

        // SAFETY: msg_pool is not null and offset is within the range
        unsafe { *((self.msg_pool as *mut usize).offset(self.in_offset as isize)) = value };

        self.in_offset += 1;
        if self.in_offset >= self.size {
            self.in_offset = 0;
        }

        if self.entry < RT_MB_ENTRY_MAX as u16 {
            self.entry += 1;
        } else {
            self.parent.unlock();
            return -(RT_EFULL as i32);
        }

        if self.parent.has_waiting() {
            self.parent.wake_one();

            self.parent.unlock();

            Cpu::get_current_scheduler().do_task_schedule();

            return RT_EOK as i32;
        }

        self.parent.unlock();

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
                &mut self.parent.parent as *mut KObjectBase as *mut rt_object
            );
        }

        self.parent.lock();

        if self.entry == self.size {
            self.parent.unlock();
            return -(RT_EFULL as i32);
        }

        if self.out_offset > 0 {
            self.out_offset -= 1;
        } else {
            self.out_offset = self.size - 1;
        }

        // SAFETY： msg_pool is not null and offset is within the range
        unsafe {
            *((self.msg_pool as *mut usize).offset(self.out_offset as isize)) = value;
        }

        self.entry += 1;

        if self.parent.has_waiting() {
            self.parent.wake_one();

            self.parent.unlock();

            Cpu::get_current_scheduler().do_task_schedule();

            return RT_EOK as i32;
        }

        self.parent.unlock();

        RT_EOK as i32
    }

    fn receive_internal(&mut self, value: &mut usize, timeout: i32, suspend_flag: u32) -> i32 {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassMailBox as u8);

        let mut timeout = timeout;
        #[allow(unused_variables)]
        let scheduler = timeout != 0;
        rt_debug_scheduler_available!(scheduler);

        let mut tick_delta = 0;

        let thread = unsafe { crate::current_thread!().unwrap().as_mut() };
        // SAFETY： hook and memory operation should be ensured safe
        unsafe {
            rt_object_hook_call!(
                rt_object_trytake_hook,
                &mut self.parent.parent as *mut KObjectBase as *mut rt_object
            );

            self.parent.lock();

            if self.entry == 0 && timeout == 0 {
                self.parent.unlock();
                return -(RT_ETIMEOUT as i32);
            }

            while self.entry == 0 {
                (*thread).error = -(RT_EINTR as i32);

                if timeout == 0 {
                    self.parent.unlock();
                    (*thread).error = -(RT_ETIMEOUT as i32);

                    return -(RT_ETIMEOUT as i32);
                }

                let ret = self
                    .parent
                    .wait(thread, self.parent.flag, suspend_flag as u32);
                if ret != RT_EOK as i32 {
                    self.parent.unlock();
                    return ret;
                }

                if timeout > 0 {
                    tick_delta = rt_tick_get();

                    (*thread).thread_timer.timer_control(
                        RT_TIMER_CTRL_SET_TIME as u32,
                        (&mut timeout) as *mut i32 as *mut c_void,
                    );
                    (*thread).thread_timer.start();
                }

                self.parent.unlock();

                Cpu::get_current_scheduler().do_task_schedule();

                if (*thread).error != RT_EOK as i32 {
                    return (*thread).error;
                }

                self.parent.lock();

                if timeout > 0 {
                    tick_delta = rt_tick_get() - tick_delta;
                    timeout -= tick_delta as i32;
                    if timeout < 0 {
                        timeout = 0;
                    }
                }
            }

            *value = *((self.msg_pool as *mut usize).offset(self.out_offset as isize));

            self.out_offset += 1;
            if self.out_offset >= self.size {
                self.out_offset = 0;
            }

            if self.entry > 0 {
                self.entry -= 1;
            }

            if !self.suspend_sender_thread.is_empty() {
                IPCObject::resume_thread(&mut self.suspend_sender_thread);

                self.parent.unlock();

                rt_object_hook_call!(
                    rt_object_take_hook,
                    &mut self.parent.parent as *mut KObjectBase as *mut rt_object
                );

                Cpu::get_current_scheduler().do_task_schedule();

                return RT_EOK as i32;
            }

            self.parent.unlock();

            rt_object_hook_call!(
                rt_object_take_hook,
                &mut self.parent.parent as *mut KObjectBase as *mut rt_object
            );

            RT_EOK as i32
        }
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
            self.parent.lock();

            self.parent.wake_all();
            IPCObject::resume_all_threads(&mut self.suspend_sender_thread);

            self.entry = 0;
            self.in_offset = 0;
            self.out_offset = 0;

            self.parent.unlock();

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
        let _ = crate::format_name!(mailbox.parent.parent.name.as_ptr(), 8);
        print!(" {:04} ", mailbox.entry);
        print!(" {:04} ", mailbox.size);
        if mailbox.parent.wait_list.is_empty() {
            println!("{}", mailbox.parent.wait_list.size());
        } else {
            print!("{}:", mailbox.parent.wait_list.size());
            let head = &mailbox.parent.wait_list;
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
