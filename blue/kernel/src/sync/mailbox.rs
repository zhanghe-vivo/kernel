use crate::{
    allocator::{rt_free, rt_malloc},
    clock::rt_tick_get,
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
    timer::{rt_timer_control, rt_timer_start, Timer},
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

use crate::sync::RawSpin;
use pinned_init::*;

/// Mailbox raw structure
#[repr(C)]
#[pin_data]
pub struct RtMailbox {
    /// Inherit from IPCObject
    #[pin]
    pub(crate) parent: IPCObject,
    /// Message pool buffer of mailbox
    pub(crate) msg_pool: *mut ffi::c_ulong,
    /// Message pool buffer size
    pub(crate) size: ffi::c_ushort,
    /// Index of messages in message pool
    pub(crate) entry: ffi::c_ushort,
    /// Input offset of the message buffer
    pub(crate) in_offset: ffi::c_ushort,
    /// Output offset of the message buffer
    pub(crate) out_offset: ffi::c_ushort,
    /// Sender thread suspended on this mailbox
    #[pin]
    pub(crate) suspend_sender_thread: ListHead,
}

impl_kobject!(RtMailbox);

impl RtMailbox {
    #[inline]
    pub fn init(
        &mut self,
        name: *const i8,
        msg_pool: *mut core::ffi::c_void,
        size: usize,
        flag: u8,
    ) {
        assert!((flag == RT_IPC_FLAG_FIFO as u8) || (flag == RT_IPC_FLAG_PRIO as u8));

        self.parent
            .init(ObjectClassType::ObjectClassMailBox as u8, name, flag);

        self.msg_pool = msg_pool as *mut ffi::c_ulong;
        self.size = size as ffi::c_ushort;
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
    pub fn new(name: *const i8, size: usize, flag: u8) -> *mut Self {
        assert!((flag == RT_IPC_FLAG_FIFO as u8) || (flag == RT_IPC_FLAG_PRIO as u8));

        rt_debug_not_in_interrupt!();

        // SAFETY: we have null ptr protection
        unsafe {
            let mb = IPCObject::new::<Self>(ObjectClassType::ObjectClassMailBox as u8, name, flag);
            if !mb.is_null() {
                (*mb).size = size as u16;
                let ptr = rt_malloc((*mb).size as usize * mem::size_of::<ffi::c_ulong>())
                    as *mut ffi::c_ulong;
                (*mb).msg_pool = ptr;
                if (*mb).msg_pool.is_null() {
                    (*mb).parent.parent.delete();
                    return null_mut();
                }

                (*mb).entry = 0;
                (*mb).in_offset = 0;
                (*mb).out_offset = 0;

                unsafe {
                    let _ = ListHead::new()
                        .__pinned_init(&mut (*mb).suspend_sender_thread as *mut ListHead);
                }
            }

            mb
        }
    }

    #[inline]
    pub fn delete(&mut self) {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassMailBox as u8);
        assert!(!self.is_static_kobject());

        rt_debug_not_in_interrupt!();

        self.parent.wake_all();
        IPCObject::resume_all_threads(&mut self.suspend_sender_thread);
        // SAFETY: null protection
        unsafe {
            if !self.msg_pool.is_null() {
                rt_free(self.msg_pool as *mut c_void);
            }
        }
        self.parent.parent.delete();
    }

    fn send_wait_internal(
        &mut self,
        value: ffi::c_ulong,
        timeout: i32,
        suspend_flag: u32,
    ) -> ffi::c_long {
        unsafe {
            let mut timeout = timeout;
            #[allow(unused_variables)]
            let scheduler = timeout != 0;
            rt_debug_scheduler_available!(scheduler);

            let mut tick_delta = 0;
            let thread = unsafe { crate::current_thread!().unwrap().as_mut() };

            rt_object_hook_call!(
                rt_object_put_hook,
                &mut self.parent.parent as *mut KObjectBase as *mut rt_object
            );

            self.parent.lock();

            if self.entry == self.size && timeout == 0 {
                self.parent.unlock();
                return -(RT_EFULL as ffi::c_long);
            }

            while self.entry == self.size {
                (*thread).error = -(RT_EINTR as ffi::c_long);

                if timeout == 0 {
                    self.parent.unlock();

                    return -(RT_EFULL as ffi::c_long);
                }

                let ret = IPCObject::suspend_thread(
                    &mut self.suspend_sender_thread,
                    thread,
                    self.parent.flag,
                    suspend_flag as u32,
                );

                if ret != RT_EOK as ffi::c_long {
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

                if (*thread).error != RT_EOK as ffi::c_long {
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

            *self.msg_pool.offset(self.in_offset as isize) = value;
            self.in_offset += 1;
            if self.in_offset >= self.size {
                self.in_offset = 0;
            }

            //unsafemonitor
            if self.entry < RT_MB_ENTRY_MAX as u16 {
                self.entry += 1;
            } else {
                self.parent.unlock();
                return -(RT_EFULL as ffi::c_long);
            }

            if self.parent.has_waiting() {
                self.parent.wake_one();

                self.parent.unlock();

                Cpu::get_current_scheduler().do_task_schedule();

                return RT_EOK as ffi::c_long;
            }

            self.parent.unlock();

            return RT_EOK as ffi::c_long;
        }
    }

    pub fn send_wait(&mut self, value: ffi::c_ulong, timeout: i32) -> ffi::c_long {
        self.send_wait_internal(value, timeout, RT_UNINTERRUPTIBLE as u32)
    }

    pub fn send_wait_interruptible(&mut self, value: ffi::c_ulong, timeout: i32) -> ffi::c_long {
        self.send_wait_internal(value, timeout, RT_INTERRUPTIBLE as u32)
    }

    pub fn send_wait_killable(&mut self, value: ffi::c_ulong, timeout: i32) -> ffi::c_long {
        self.send_wait_internal(value, timeout, RT_KILLABLE as u32)
    }

    pub fn send(&mut self, value: ffi::c_ulong) -> ffi::c_long {
        self.send_wait(value, 0)
    }

    pub fn send_interruptible(&mut self, value: ffi::c_ulong) -> ffi::c_long {
        self.send_wait_interruptible(value, 0)
    }

    pub fn send_killable(&mut self, value: ffi::c_ulong) -> ffi::c_long {
        self.send_wait_killable(value, 0)
    }

    pub fn urgent(&mut self, value: ffi::c_ulong) -> ffi::c_long {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassMailBox as u8);

        // SAFETY： hook and memory operation should be ensured safe
        unsafe {
            rt_object_hook_call!(
                rt_object_put_hook,
                &mut self.parent.parent as *mut KObjectBase as *mut rt_object
            );

            self.parent.lock();

            if self.entry == self.size {
                self.parent.unlock();
                return -(RT_EFULL as ffi::c_long);
            }

            if self.out_offset > 0 {
                self.out_offset -= 1;
            } else {
                self.out_offset = self.size - 1;
            }

            *self.msg_pool.offset(self.out_offset as isize) = value;

            self.entry += 1;

            if self.parent.has_waiting() {
                self.parent.wake_one();

                self.parent.unlock();

                Cpu::get_current_scheduler().do_task_schedule();

                return RT_EOK as ffi::c_long;
            }

            self.parent.unlock();

            RT_EOK as ffi::c_long
        }
    }

    fn receive_internal(
        &mut self,
        value: *mut core::ffi::c_ulong,
        timeout: i32,
        suspend_flag: u32,
    ) -> ffi::c_long {
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
                return -(RT_ETIMEOUT as ffi::c_long);
            }

            while self.entry == 0 {
                (*thread).error = -(RT_EINTR as ffi::c_long);

                if timeout == 0 {
                    self.parent.unlock();
                    (*thread).error = -(RT_ETIMEOUT as ffi::c_long);

                    return -(RT_ETIMEOUT as ffi::c_long);
                }

                let ret = self
                    .parent
                    .wait(thread, self.parent.flag, suspend_flag as u32);
                if ret != RT_EOK as ffi::c_long {
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

                if (*thread).error != RT_EOK as ffi::c_long {
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

            //unsafemonitor
            *value = *self.msg_pool.offset(self.out_offset as isize);

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

                return RT_EOK as ffi::c_long;
            }

            self.parent.unlock();

            rt_object_hook_call!(
                rt_object_take_hook,
                &mut self.parent.parent as *mut KObjectBase as *mut rt_object
            );

            RT_EOK as ffi::c_long
        }
    }

    pub fn receive(&mut self, value: *mut core::ffi::c_ulong, timeout: i32) -> ffi::c_long {
        self.receive_internal(value, timeout, RT_UNINTERRUPTIBLE)
    }

    pub fn receive_interruptible(
        &mut self,
        value: *mut core::ffi::c_ulong,
        timeout: i32,
    ) -> ffi::c_long {
        self.receive_internal(value, timeout, RT_INTERRUPTIBLE)
    }

    pub fn receive_killable(
        &mut self,
        value: *mut core::ffi::c_ulong,
        timeout: i32,
    ) -> ffi::c_long {
        self.receive_internal(value, timeout, RT_KILLABLE)
    }

    pub fn control(&mut self, cmd: i32, _arg: *mut core::ffi::c_void) -> ffi::c_long {
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

            return RT_EOK as ffi::c_long;
        }

        RT_EOK as ffi::c_long
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

    (*mb).init(name, msgpool, size as usize, flag);

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
    RtMailbox::new(name, size as usize, flag)
}

#[cfg(all(feature = "RT_USING_MAILBOX", feature = "RT_USING_HEAP"))]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_delete(mb: *mut RtMailbox) -> rt_err_t {
    assert!(!mb.is_null());

    (*mb).delete();

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
    (*mb).send_wait(value, timeout)
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_send_wait_interruptible(
    mb: *mut RtMailbox,
    value: rt_ubase_t,
    timeout: rt_int32_t,
) -> rt_err_t {
    assert!(!mb.is_null());
    (*mb).send_wait_interruptible(value, timeout)
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_send_wait_killable(
    mb: *mut RtMailbox,
    value: rt_ubase_t,
    timeout: rt_int32_t,
) -> rt_err_t {
    assert!(!mb.is_null());
    (*mb).send_wait_killable(value, timeout)
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_send(mb: *mut RtMailbox, value: rt_ubase_t) -> rt_err_t {
    assert!(!mb.is_null());
    (*mb).send(value)
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_send_interruptible(
    mb: *mut RtMailbox,
    value: rt_ubase_t,
) -> rt_err_t {
    assert!(!mb.is_null());
    (*mb).send_interruptible(value)
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_send_killable(mb: *mut RtMailbox, value: rt_ubase_t) -> rt_err_t {
    assert!(!mb.is_null());
    (*mb).send_killable(value)
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_urgent(mb: *mut RtMailbox, value: rt_ubase_t) -> rt_err_t {
    assert!(!mb.is_null());
    (*mb).urgent(value)
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_recv(
    mb: *mut RtMailbox,
    value: *mut core::ffi::c_ulong,
    timeout: rt_int32_t,
) -> rt_err_t {
    assert!(!mb.is_null());
    (*mb).receive(value, timeout)
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_recv_interruptible(
    mb: *mut RtMailbox,
    value: *mut core::ffi::c_ulong,
    timeout: rt_int32_t,
) -> rt_err_t {
    assert!(!mb.is_null());
    (*mb).receive_interruptible(value, timeout)
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_recv_killable(
    mb: *mut RtMailbox,
    value: *mut core::ffi::c_ulong,
    timeout: rt_int32_t,
) -> rt_err_t {
    assert!(!mb.is_null());
    (*mb).receive_killable(value, timeout)
}

#[cfg(feature = "RT_USING_MAILBOX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mb_control(
    mb: *mut RtMailbox,
    cmd: core::ffi::c_int,
    _arg: *mut core::ffi::c_void,
) -> rt_err_t {
    assert!(!mb.is_null());
    (*mb).control(cmd, _arg)
}

#[pin_data]
pub struct MailBox {
    #[pin]
    mb_ptr: *mut RtMailbox,
    #[pin]
    _pin: PhantomPinned,
}

unsafe impl Send for MailBox {}
unsafe impl Sync for MailBox {}

impl MailBox {
    pub fn new(name: &str, size: usize) -> Result<Self, Error> {
        let result = unsafe {
            rt_mb_create(
                name.as_ptr() as *const c_char,
                size as rt_size_t,
                RT_IPC_FLAG_PRIO as u8,
            )
        };
        if result.is_null() {
            Err(Error::from_errno(RT_ERROR as i32))
        } else {
            Ok(Self {
                mb_ptr: result,
                _pin: PhantomPinned {},
            })
        }
    }

    pub fn delete(self) -> Result<(), Error> {
        let result = unsafe { rt_mb_delete(self.mb_ptr) };
        if result == RT_EOK as rt_err_t {
            Ok(())
        } else {
            Err(Error::from_errno(result))
        }
    }

    pub fn send(&self, set: u32) -> Result<(), Error> {
        self.send_wait(set, 0)
    }

    pub fn send_wait(&self, set: u32, timeout: rt_int32_t) -> Result<(), Error> {
        let result = unsafe { rt_mb_send_wait(self.mb_ptr, set, timeout) };
        if result == RT_EOK as rt_err_t {
            Ok(())
        } else {
            Err(Error::from_errno(result))
        }
    }

    pub fn send_interruptible(&self, set: u32) -> Result<(), Error> {
        let result = unsafe { rt_mb_send_interruptible(self.mb_ptr, set as rt_ubase_t) };
        if result == RT_EOK as rt_err_t {
            Ok(())
        } else {
            Err(Error::from_errno(result))
        }
    }

    pub fn send_killable(&self, set: u32) -> Result<(), Error> {
        let result = unsafe { rt_mb_send_killable(self.mb_ptr, set as rt_ubase_t) };
        if result == RT_EOK as rt_err_t {
            Ok(())
        } else {
            Err(Error::from_errno(result))
        }
    }

    pub fn receive(&self, timeout: i32) -> Result<u32, Error> {
        let mut retmsg = 0 as core::ffi::c_ulong;
        let result = unsafe { rt_mb_recv(self.mb_ptr, &mut retmsg, timeout) };
        if result == RT_EOK as rt_err_t {
            Ok(retmsg as u32)
        } else {
            Err(Error::from_errno(result))
        }
    }

    pub fn receive_interruptible(&self, timeout: i32) -> Result<u32, Error> {
        let mut retmsg = 0 as core::ffi::c_ulong;
        let result = unsafe { rt_mb_recv_interruptible(self.mb_ptr, &mut retmsg, timeout) };
        if result == RT_EOK as rt_err_t {
            Ok(retmsg as u32)
        } else {
            Err(Error::from_errno(result))
        }
    }

    pub fn receive_killable(&self, timeout: i32) -> Result<u32, Error> {
        let mut retmsg = 0 as core::ffi::c_ulong;
        let result = unsafe { rt_mb_recv_killable(self.mb_ptr, &mut retmsg, timeout) };
        if result == RT_EOK as i32 {
            Ok(retmsg as u32)
        } else {
            Err(Error::from_errno(result))
        }
    }
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
