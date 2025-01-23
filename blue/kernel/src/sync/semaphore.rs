use crate::{
    clock::WAITING_FOREVER, cpu::Cpu, error::code, impl_kobject, object::*, sync::ipc_common::*,
    thread::SuspendFlag, timer::TimerControlAction,
};
use core::{ffi::c_void, marker::PhantomPinned, mem, pin::Pin, ptr::null_mut};

use crate::alloc::boxed::Box;
use core::cell::UnsafeCell;
use kernel::{fmt, str::CString};

use pinned_init::{pin_data, pin_init, pin_init_from_closure, pinned_drop, InPlaceInit, PinInit};

#[pin_data(PinnedDrop)]
pub struct KSemaphore {
    #[pin]
    raw: UnsafeCell<Semaphore>,
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
        fn init_raw(value: u16) -> impl PinInit<UnsafeCell<Semaphore>> {
            let init = move |slot: *mut UnsafeCell<Semaphore>| {
                let slot: *mut Semaphore = slot.cast();
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
pub struct Semaphore {
    /// Inherit from KObject
    #[pin]
    pub(crate) parent: KObjectBase,
    /// SysQueue for semaphore
    #[pin]
    pub(crate) inner_queue: SysQueue,
}

impl_kobject!(Semaphore);

impl Semaphore {
    #[inline]
    pub fn new(name: [i8; NAME_MAX], value: u16, waiting_mode: u8) -> impl PinInit<Self> {
        assert!(
            (waiting_mode == IPC_WAIT_MODE_FIFO as u8)
                || (waiting_mode == IPC_WAIT_MODE_PRIO as u8)
        );

        crate::debug_not_in_interrupt!();

        let init = move |slot: *mut Self| unsafe {
            let cur_ref = &mut *slot;
            let _ = KObjectBase::new(ObjectClassType::ObjectClassSemaphore as u8, name)
                .__pinned_init(&mut cur_ref.parent as *mut KObjectBase);
            let _ = SysQueue::new(
                mem::size_of::<u32>(),
                value as usize,
                IPC_SYS_QUEUE_STUB,
                waiting_mode as u32,
            )
            .__pinned_init(&mut cur_ref.inner_queue as *mut SysQueue);
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
        self.inner_queue.lock();
        self.inner_queue.dequeue_waiter.wake_all();
        self.inner_queue.unlock();
        if self.is_static_kobject() {
            self.parent.detach();
        }
    }

    #[inline]
    pub fn new_raw(name: *const i8, value: u16, flag: u8) -> *mut Self {
        let semaphore = Box::pin_init(Semaphore::new(char_ptr_to_array(name), value, flag));
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

        crate::debug_not_in_interrupt!();

        self.inner_queue.lock();
        self.inner_queue.dequeue_waiter.wake_all();
        self.inner_queue.unlock();

        self.parent.delete();
    }

    pub(crate) fn count(&self) -> usize {
        self.inner_queue.item_in_queue
    }

    pub fn take_internal(&mut self, timeout: i32, pending_mode: u32) -> i32 {
        let mut time_out = timeout as i32;
        assert_eq!(
            self.type_name(),
            ObjectClassType::ObjectClassSemaphore as u8
        );

        #[allow(unused_variables)]
        let check = self.count() == 0 && timeout != 0;
        crate::debug_scheduler_available!(check);

        self.inner_queue.lock();

        if self.inner_queue.pop_stub() {
            self.inner_queue.unlock();
        } else {
            if timeout == 0 {
                self.inner_queue.unlock();

                /* FIXME: -2 is as expected, while C -code::ETIMEDOUT is -116. */
                return -116; //(code::ETIMEDOUT as rt_err_t);
            } else {
                let thread_ptr = crate::current_thread_ptr!();
                if thread_ptr.is_null() {
                    return code::ERROR.to_errno();
                }

                // SAFETY: thread_ptr is null checked
                let thread = unsafe { &mut *thread_ptr };

                thread.error = code::EINTR;

                let ret = self.inner_queue.dequeue_waiter.wait(thread, pending_mode);
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
            }
        }

        code::EOK.to_errno()
    }

    pub(crate) fn take(&mut self) -> i32 {
        self.take_internal(WAITING_FOREVER as i32, SuspendFlag::Uninterruptible as u32)
    }

    pub fn take_wait(&mut self, timeout: i32) -> i32 {
        self.take_internal(timeout, SuspendFlag::Uninterruptible as u32)
    }

    pub fn try_take(&mut self) -> i32 {
        self.take_wait(0)
    }

    #[allow(dead_code)]
    pub(crate) fn take_with_pending(&mut self, timeout: i32, pending_mode: u32) -> i32 {
        self.take_internal(timeout, pending_mode)
    }

    pub fn release(&mut self) -> i32 {
        assert_eq!(
            self.type_name(),
            ObjectClassType::ObjectClassSemaphore as u8
        );

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
                return code::ENOSPC.to_errno();
            }
        }

        self.inner_queue.unlock();

        if need_schedule {
            Cpu::get_current_scheduler().do_task_schedule();
        }

        code::EOK.to_errno()
    }

    pub fn reset(&mut self, value: u32) -> i32 {
        assert_eq!(
            self.type_name(),
            ObjectClassType::ObjectClassSemaphore as u8
        );

        self.inner_queue.lock();

        self.inner_queue.dequeue_waiter.wake_all();

        self.inner_queue.reset_stub(value as usize);

        self.inner_queue.unlock();

        Cpu::get_current_scheduler().do_task_schedule();

        code::EOK.to_errno()
    }
}

/// bindgen for Semaphore
#[allow(improper_ctypes_definitions)]
#[no_mangle]
pub extern "C" fn bindgen_sem(_sem: Semaphore) {
    0;
}
