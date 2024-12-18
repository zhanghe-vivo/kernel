use crate::{
    cpu::Cpu,
    error::code,
    impl_kobject, list_head_for_each,
    object::*,
    print, println,
    rt_bindings::{
        rt_debug_not_in_interrupt, rt_debug_scheduler_available, rt_err_t, rt_int32_t, rt_object,
        rt_object_hook_call, rt_uint32_t, rt_uint8_t, RT_EFULL, RT_EOK, RT_ERROR, RT_INTERRUPTIBLE,
        RT_IPC_CMD_RESET, RT_KILLABLE, RT_TIMER_CTRL_SET_TIME, RT_UNINTERRUPTIBLE,
        RT_WAITING_FOREVER,
    },
    sync::ipc_common::*,
    thread::RtThread,
};
use blue_infra::list::doubly_linked_list::ListHead;
use core::{ffi::c_void, marker::PhantomPinned, mem, pin::Pin, ptr::null_mut};

use crate::alloc::boxed::Box;
use core::cell::UnsafeCell;
use kernel::{fmt, str::CString};

use pinned_init::{pin_data, pin_init, pin_init_from_closure, pinned_drop, InPlaceInit, PinInit};

#[pin_data(PinnedDrop)]
pub struct KSemaphore {
    #[pin]
    raw: UnsafeCell<RtSemaphore>,
    #[pin]
    pin: PhantomPinned,
}

unsafe impl Send for KSemaphore {}
unsafe impl Sync for KSemaphore {}

#[pinned_drop]
impl PinnedDrop for KSemaphore {
    fn drop(self: Pin<&mut Self>) {
        unsafe {
            (*self.raw.get()).detach();
        }
    }
}

impl KSemaphore {
    pub fn new(value: u32) -> Pin<Box<Self>> {
        fn init_raw(value: u16) -> impl PinInit<UnsafeCell<RtSemaphore>> {
            let init = move |slot: *mut UnsafeCell<RtSemaphore>| {
                let slot: *mut RtSemaphore = slot.cast();
                unsafe {
                    let cur_ref = &mut *slot;

                    if let Ok(s) = CString::try_from_fmt(fmt!("{:p}", slot)) {
                        cur_ref.init(s.as_ptr() as *const i8, value, IPC_WAIT_MODE_PRIO as u8);
                    } else {
                        let default = "default";
                        cur_ref.init(
                            default.as_ptr() as *const i8,
                            value,
                            IPC_WAIT_MODE_PRIO as u8,
                        );
                    }
                }
                Ok(())
            };
            unsafe { pin_init_from_closure(init) }
        }

        Box::pin_init(pin_init!(Self {
            raw<-init_raw(value as u16),
            pin: PhantomPinned,
        }))
        .unwrap()
    }

    pub fn acquire(&self) -> KSemaphoreGuard<'_> {
        unsafe {
            (*self.raw.get()).take();
        };
        KSemaphoreGuard { sem: self }
    }
}
pub struct KSemaphoreGuard<'a> {
    sem: &'a KSemaphore,
}

impl<'a> Drop for KSemaphoreGuard<'a> {
    fn drop(&mut self) {
        unsafe { (*self.sem.raw.get()).release() };
    }
}

/// Semaphore raw structure
#[repr(C)]
#[pin_data]
pub struct RtSemaphore {
    /// Inherit from KObject
    #[pin]
    pub(crate) parent: KObjectBase,
    /// SysQueue for semaphore
    #[pin]
    pub(crate) inner_queue: RtSysQueue,
}

impl_kobject!(RtSemaphore);

impl RtSemaphore {
    #[inline]
    pub fn new(name: [i8; NAME_MAX], value: u16, waiting_mode: u8) -> impl PinInit<Self> {
        assert!(
            (waiting_mode == IPC_WAIT_MODE_FIFO as u8)
                || (waiting_mode == IPC_WAIT_MODE_PRIO as u8)
        );

        rt_debug_not_in_interrupt!();

        let init = move |slot: *mut Self| unsafe {
            let cur_ref = &mut *slot;
            let _ = KObjectBase::new(ObjectClassType::ObjectClassSemaphore as u8, name)
                .__pinned_init(&mut cur_ref.parent as *mut KObjectBase);
            let _ = RtSysQueue::new(
                mem::size_of::<u32>(),
                value as usize,
                IPC_SYS_QUEUE_STUB,
                waiting_mode as u32,
            )
            .__pinned_init(&mut cur_ref.inner_queue as *mut RtSysQueue);
            cur_ref.inner_queue.reset_stub(value as usize);
            Ok(())
        };
        unsafe { pin_init_from_closure(init) }
    }

    #[inline]
    pub fn init(&mut self, name: *const i8, value: u16, waiting_mode: u8) {
        assert!(
            (waiting_mode == IPC_WAIT_MODE_FIFO as u8)
                || (waiting_mode == IPC_WAIT_MODE_PRIO as u8)
        );
        self.parent
            .init(ObjectClassType::ObjectClassSemaphore as u8, name);
        self.init_internal(value, waiting_mode);
    }

    #[inline]
    pub fn init_dyn(&mut self, name: *const i8, value: u16, waiting_mode: u8) {
        assert!(
            (waiting_mode == IPC_WAIT_MODE_FIFO as u8)
                || (waiting_mode == IPC_WAIT_MODE_PRIO as u8)
        );
        self.parent
            .init_dyn(ObjectClassType::ObjectClassSemaphore as u8, name);
        self.init_internal(value, waiting_mode);
    }

    #[inline]
    pub fn init_internal(&mut self, value: u16, waiting_mode: u8) {
        assert!(
            (waiting_mode == IPC_WAIT_MODE_FIFO as u8)
                || (waiting_mode == IPC_WAIT_MODE_PRIO as u8)
        );
        self.inner_queue.init(
            null_mut(),
            mem::size_of::<u32>(),
            value as usize,
            IPC_SYS_QUEUE_STUB,
            waiting_mode as u32,
        );
        self.inner_queue.reset_stub(value as usize);
    }

    #[inline]
    pub fn detach(&mut self) {
        self.inner_queue.dequeue_waiter.inner_locked_wake_all();
        if self.is_static_kobject() {
            self.parent.detach();
        }
    }

    #[inline]
    pub fn new_raw(name: *const i8, value: u16, flag: u8) -> *mut Self {
        let semaphore = Box::pin_init(RtSemaphore::new(char_ptr_to_array(name), value, flag));
        match semaphore {
            Ok(sem) => unsafe { Box::leak(Pin::into_inner_unchecked(sem)) },
            Err(_) => return null_mut(),
        }
    }

    #[inline]
    pub fn delete_raw(&mut self) {
        assert_eq!(
            self.type_name(),
            ObjectClassType::ObjectClassSemaphore as u8
        );
        assert!(!self.is_static_kobject());

        rt_debug_not_in_interrupt!();

        self.inner_queue.dequeue_waiter.inner_locked_wake_all();
        self.parent.delete();
    }

    pub(crate) fn count(&self) -> usize {
        self.inner_queue.item_in_queue
    }

    fn take_internal(&mut self, timeout: i32, pending_mode: u32) -> i32 {
        let mut time_out = timeout as i32;
        assert_eq!(
            self.type_name(),
            ObjectClassType::ObjectClassSemaphore as u8
        );
        // SAFETY: only one thread can take the semaphore
        unsafe {
            rt_object_hook_call!(
                rt_object_trytake_hook,
                &mut self.parent as *mut KObjectBase as *mut rt_object
            );
        }

        #[allow(unused_variables)]
        let check = self.count() == 0 && timeout != 0;
        rt_debug_scheduler_available!(check);

        self.inner_queue.lock();

        if self.inner_queue.pop_stub() {
            self.inner_queue.unlock();
        } else {
            if timeout == 0 {
                self.inner_queue.unlock();

                /* FIXME: -2 is as expected, while C -RT_ETIMEOUT is -116. */
                return -116; //(RT_ETIMEOUT as rt_err_t);
            } else {
                let thread_ptr = crate::current_thread_ptr!();
                if thread_ptr.is_null() {
                    return -(RT_ERROR as i32);
                }

                // SAFETY: thread_ptr is null checked
                let thread = unsafe { &mut *thread_ptr };

                thread.error = code::EINTR;

                let ret = self.inner_queue.dequeue_waiter.wait(thread, pending_mode);
                if ret != RT_EOK as i32 {
                    self.inner_queue.unlock();
                    return ret;
                }

                if timeout > 0 {
                    thread.thread_timer.timer_control(
                        RT_TIMER_CTRL_SET_TIME as u32,
                        (&mut time_out) as *mut i32 as *mut c_void,
                    );
                    thread.thread_timer.start();
                }

                self.inner_queue.unlock();

                Cpu::get_current_scheduler().do_task_schedule();

                if thread.error != code::EOK {
                    return thread.error.to_errno();
                }
            }
        }
        // SAFETY: only one thread can take the semaphore
        unsafe {
            rt_object_hook_call!(
                rt_object_take_hook,
                &mut self.parent as *mut KObjectBase as *mut rt_object
            );
        }

        RT_EOK as i32
    }

    pub(crate) fn take(&mut self) -> i32 {
        self.take_internal(RT_WAITING_FOREVER, RT_UNINTERRUPTIBLE as u32)
    }

    pub(crate) fn take_wait(&mut self, timeout: i32) -> i32 {
        self.take_internal(timeout, RT_UNINTERRUPTIBLE as u32)
    }

    pub(crate) fn try_take(&mut self) -> i32 {
        self.take_wait(0)
    }

    #[allow(dead_code)]
    pub(crate) fn take_with_pending(&mut self, timeout: i32, pending_mode: u32) -> i32 {
        self.take_internal(timeout, pending_mode)
    }

    pub(crate) fn release(&mut self) -> i32 {
        assert_eq!(
            self.type_name(),
            ObjectClassType::ObjectClassSemaphore as u8
        );
        // SAFETY: only one thread can take the semaphore
        unsafe {
            rt_object_hook_call!(
                rt_object_put_hook,
                &mut self.parent as *mut KObjectBase as *mut rt_object
            );
        }

        let mut need_schedule = false;
        self.inner_queue.lock();

        if !self.inner_queue.dequeue_waiter.is_empty() {
            self.inner_queue.dequeue_waiter.wake();
            need_schedule = true;
        } else {
            if self.count() < IPC_SEMAPHORE_COUNT_MAX as usize {
                self.inner_queue.force_push_stub();
            } else {
                self.inner_queue.unlock();
                return -(RT_EFULL as i32);
            }
        }

        self.inner_queue.unlock();

        if need_schedule {
            Cpu::get_current_scheduler().do_task_schedule();
        }

        RT_EOK as i32
    }

    pub(crate) fn reset(&mut self, value: u32) -> i32 {
        assert_eq!(
            self.type_name(),
            ObjectClassType::ObjectClassSemaphore as u8
        );

        self.inner_queue.lock();

        self.inner_queue.dequeue_waiter.inner_locked_wake_all();

        self.inner_queue.reset_stub(value as usize);

        self.inner_queue.unlock();

        Cpu::get_current_scheduler().do_task_schedule();

        RT_EOK as i32
    }
}

#[cfg(feature = "RT_USING_SEMAPHORE")]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_init(
    sem: *mut RtSemaphore,
    name: *const core::ffi::c_char,
    value: rt_uint32_t,
    flag: rt_uint8_t,
) -> rt_err_t {
    assert!(!sem.is_null());
    (*sem).init(name, value as u16, flag);
    RT_EOK as rt_err_t
}
#[cfg(feature = "RT_USING_SEMAPHORE")]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_detach(sem: *mut RtSemaphore) -> rt_err_t {
    assert!(!sem.is_null());
    assert_eq!(
        (*sem).type_name(),
        ObjectClassType::ObjectClassSemaphore as u8
    );
    assert!((*sem).is_static_kobject());
    (*sem).detach();
    RT_EOK as rt_err_t
}
#[cfg(all(feature = "RT_USING_SEMAPHORE", feature = "RT_USING_HEAP"))]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_create(
    name: *const core::ffi::c_char,
    value: rt_uint32_t,
    flag: rt_uint8_t,
) -> *mut RtSemaphore {
    RtSemaphore::new_raw(name, value as u16, flag)
}

#[cfg(all(feature = "RT_USING_SEMAPHORE", feature = "RT_USING_HEAP"))]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_delete(sem: *mut RtSemaphore) -> rt_err_t {
    assert!(!sem.is_null());
    (*sem).delete_raw();
    RT_EOK as rt_err_t
}

#[cfg(feature = "RT_USING_SEMAPHORE")]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_take(sem: *mut RtSemaphore, time: rt_int32_t) -> rt_err_t {
    assert!(!sem.is_null());
    (*sem).take_wait(time)
}

#[cfg(feature = "RT_USING_SEMAPHORE")]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_take_interruptible(
    sem: *mut RtSemaphore,
    time: rt_int32_t,
) -> rt_err_t {
    assert!(!sem.is_null());
    (*sem).take_internal(time, RT_INTERRUPTIBLE as u32)
}

#[cfg(feature = "RT_USING_SEMAPHORE")]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_take_killable(sem: *mut RtSemaphore, time: rt_int32_t) -> rt_err_t {
    assert!(!sem.is_null());
    (*sem).take_internal(time, RT_KILLABLE as u32)
}

#[cfg(feature = "RT_USING_SEMAPHORE")]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_trytake(sem: *mut RtSemaphore) -> rt_err_t {
    assert!(!sem.is_null());
    (*sem).try_take()
}

#[cfg(feature = "RT_USING_SEMAPHORE")]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_release(sem: *mut RtSemaphore) -> rt_err_t {
    assert!(!sem.is_null());
    (*sem).release()
}

#[cfg(feature = "RT_USING_SEMAPHORE")]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_control(
    sem: *mut RtSemaphore,
    cmd: i32,
    arg: *const c_void,
) -> rt_err_t {
    assert!(!sem.is_null());

    if cmd == RT_IPC_CMD_RESET as i32 {
        (*sem).reset(arg as u32);
        return RT_EOK as i32;
    }

    -(RT_ERROR as rt_err_t)
}

#[no_mangle]
#[allow(unused_unsafe)]
pub extern "C" fn rt_sem_info() {
    let callback_forword = || {
        println!("semaphor v   suspend thread");
        println!("-------- --- --------------");
    };
    let callback = |node: &ListHead| unsafe {
        let sem =
            &*(crate::list_head_entry!(node.as_ptr(), KObjectBase, list) as *const RtSemaphore);
        let _ = crate::format_name!(sem.parent.name.as_ptr(), 8);
        print!(" {:03} ", sem.count());
        if sem.inner_queue.dequeue_waiter.is_empty() {
            println!("{}", sem.inner_queue.dequeue_waiter.count());
        } else {
            print!("{}:", sem.inner_queue.dequeue_waiter.count());
            let head = &sem.inner_queue.dequeue_waiter.working_queue;
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
        ObjectClassType::ObjectClassSemaphore as u8,
    );
}
