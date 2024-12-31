use crate::{
    clock::WAITING_FOREVER,
    cpu::Cpu,
    error::code,
    impl_kobject,
    object::{KObjectBase, KernelObject, ObjectClassType, NAME_MAX},
    sync::{ipc_common::*, lock::mutex::RtMutex, semaphore::RtSemaphore, RawSpin},
    thread::SuspendFlag,
    timer::TimerControlAction,
};
use blue_infra::list::doubly_linked_list::ListHead;
use core::{ffi::c_void, ptr::null_mut};
use pinned_init::*;

/// Condition variable raw structure
#[repr(C)]
#[pin_data]
pub struct RtCondVar {
    /// Inherit from KObjectBase
    #[pin]
    pub(crate) parent: KObjectBase,
    /// Spin lock
    pub(crate) spinlock: RawSpin,
    /// Inner semaphore condvar uses
    #[pin]
    pub(crate) inner_sem: RtSemaphore,
}

impl_kobject!(RtCondVar);

impl RtCondVar {
    #[inline]
    pub(crate) fn new(name: [i8; NAME_MAX], waiting_mode: u8) -> impl PinInit<Self> {
        assert!(
            (waiting_mode == IPC_WAIT_MODE_FIFO as u8)
                || (waiting_mode == IPC_WAIT_MODE_PRIO as u8)
        );

        crate::debug_not_in_interrupt!();

        pin_init!(Self {
            parent<-KObjectBase::new(ObjectClassType::ObjectClassCondVar as u8, name),
            spinlock: RawSpin::new(),
            inner_sem<-RtSemaphore::new(name, 0, waiting_mode as u8)
        })
    }

    #[allow(dead_code)]
    #[inline]
    pub fn init(&mut self, name: *const i8, waiting_mode: u8) {
        assert!(
            (waiting_mode == IPC_WAIT_MODE_FIFO as u8)
                || (waiting_mode == IPC_WAIT_MODE_PRIO as u8)
        );
        self.parent
            .init(ObjectClassType::ObjectClassCondVar as u8, name);
        self.spinlock = RawSpin::new();
        self.inner_sem.init_dyn(name, 0, waiting_mode as u8);
    }

    #[inline]
    pub(crate) fn init_dyn(&mut self, name: *const i8, waiting_mode: u8) {
        assert!(
            (waiting_mode == IPC_WAIT_MODE_FIFO as u8)
                || (waiting_mode == IPC_WAIT_MODE_PRIO as u8)
        );
        self.parent
            .init_dyn(ObjectClassType::ObjectClassCondVar as u8, name);
        self.spinlock = RawSpin::new();
        self.inner_sem.init_dyn(name, 0, waiting_mode as u8);
    }

    #[inline]
    pub fn detach(&mut self) {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassCondVar as u8);

        self.inner_sem.detach();
        if self.is_static_kobject() {
            self.parent.detach();
        }
    }
    #[inline]
    pub fn wait(&mut self, mutex: &mut RtMutex) -> i32 {
        self.wait_timeout(
            mutex,
            SuspendFlag::Uninterruptible as u32,
            WAITING_FOREVER as i32,
        )
    }

    #[inline]
    pub(crate) fn wait_timeout(
        &mut self,
        mutex: &mut RtMutex,
        pending_mode: u32,
        timeout: i32,
    ) -> i32 {
        let mut time_out = timeout;
        let mut result = code::EOK.to_errno();

        let thread_ptr = crate::current_thread_ptr!();
        if mutex.owner != thread_ptr {
            return code::ERROR.to_errno();
        }

        self.spinlock.lock();

        if self.inner_sem.inner_queue.item_in_queue > 0 {
            self.inner_sem.inner_queue.pop_stub();
            self.spinlock.unlock();
        } else {
            if time_out == 0 {
                self.spinlock.unlock();
                if mutex.unlock() != code::EOK.to_errno() {
                    return code::ERROR.to_errno();
                }
                return code::ETIMEOUT.to_errno();
            } else {
                crate::debug_in_thread_context!();

                let thread = unsafe { &mut (*thread_ptr) };

                thread.error = code::EOK;
                self.inner_sem
                    .inner_queue
                    .dequeue_waiter
                    .wait(thread, pending_mode);

                if time_out > 0 {
                    thread.thread_timer.timer_control(
                        TimerControlAction::SetTime,
                        (&mut time_out) as *mut i32 as *mut c_void,
                    );

                    thread.thread_timer.start();
                }

                if mutex.unlock() != code::EOK.to_errno() {
                    self.spinlock.unlock();
                    return code::ERROR.to_errno();
                }

                self.spinlock.unlock();

                Cpu::get_current_scheduler().do_task_schedule();

                if thread.error != code::EOK {
                    result = thread.error.to_errno();
                }
            }
        }
        mutex.lock();
        return result;
    }

    #[inline]
    pub fn notify(&mut self) -> i32 {
        self.spinlock.lock();
        if !self.inner_sem.inner_queue.dequeue_waiter.is_empty() {
            self.spinlock.unlock();
            self.inner_sem.release();
            return code::EOK.to_errno();
        }

        self.spinlock.unlock();
        code::EOK.to_errno()
    }

    #[inline]
    pub fn notify_all(&mut self) -> i32 {
        #[allow(unused_assignments)]
        let mut result = code::EOK.to_errno();
        loop {
            result = self.inner_sem.try_take();
            if result == code::ETIMEOUT.to_errno() {
                self.inner_sem.release();
            } else if result == code::EOK.to_errno() {
                break;
            } else {
                return code::EINVAL.to_errno();
            }
        }

        code::EOK.to_errno()
    }
}

/// bindgen for RtCondVar
#[allow(improper_ctypes_definitions)]
#[no_mangle]
pub extern "C" fn bindgen_condvar(_condvar: RtCondVar) {
    0;
}
