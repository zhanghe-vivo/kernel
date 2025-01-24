use crate::{
    clock::WAITING_FOREVER,
    cpu::Cpu,
    error::code,
    impl_kobject,
    object::{KObjectBase, KernelObject, ObjectClassType, NAME_MAX},
    sync::{ipc_common::*, lock::mutex::Mutex},
    thread::SuspendFlag,
    timer::TimerControlAction,
};
use blue_infra::list::doubly_linked_list::ListHead;
use core::{ffi::c_void, ptr::null_mut};
use pinned_init::*;

///
/// Condition variable raw structure
/// 
#[repr(C)]
#[pin_data]
pub struct CondVar {
    /// Inherit from KObjectBase
    #[pin]
    pub(crate) parent: KObjectBase,
    /// Inner queue condvar uses
    #[pin]
    pub(crate) inner_queue: SysQueue,
}

impl_kobject!(CondVar);

impl CondVar {
    #[inline]
    pub(crate) fn new(name: [i8; NAME_MAX], waiting_mode: u8) -> impl PinInit<Self> {
        assert!(
            (waiting_mode == IPC_WAIT_MODE_FIFO as u8)
                || (waiting_mode == IPC_WAIT_MODE_PRIO as u8)
        );

        crate::debug_not_in_interrupt!();

        pin_init!(Self {
            parent<-KObjectBase::new(ObjectClassType::ObjectClassCondVar as u8, name),
            inner_queue<-SysQueue::new(
                core::mem::size_of::<u32>(),
                0,
                IPC_SYS_QUEUE_STUB,
                waiting_mode as u32,
            )
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
        self.inner_queue.init(
            null_mut(),
            core::mem::size_of::<u32>(),
            0,
            IPC_SYS_QUEUE_STUB,
            waiting_mode as u32,
        );
    }

    #[inline]
    pub(crate) fn init_dyn(&mut self, name: *const i8, waiting_mode: u8) {
        assert!(
            (waiting_mode == IPC_WAIT_MODE_FIFO as u8)
                || (waiting_mode == IPC_WAIT_MODE_PRIO as u8)
        );
        self.parent
            .init_dyn(ObjectClassType::ObjectClassCondVar as u8, name);
        self.inner_queue.init(
            null_mut(),
            core::mem::size_of::<u32>(),
            0,
            IPC_SYS_QUEUE_STUB,
            waiting_mode as u32,
        );
    }

    #[inline]
    pub fn detach(&mut self) {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassCondVar as u8);
        self.inner_queue.wake_all_dequeue_stub_locked();
        if self.is_static_kobject() {
            self.parent.detach();
        }
    }

    #[inline]
    pub fn wait(&mut self, mutex: &mut Mutex) -> i32 {
        self.wait_timeout(
            mutex,
            SuspendFlag::Uninterruptible as u32,
            WAITING_FOREVER as i32,
        )
    }

    #[inline]
    pub(crate) fn wait_timeout(
        &mut self,
        mutex: &mut Mutex,
        pending_mode: u32,
        timeout: i32,
    ) -> i32 {
        let mut time_out = timeout;
        let mut result = code::EOK.to_errno();

        let thread_ptr = crate::current_thread_ptr!();
        if mutex.owner != thread_ptr {
            return code::ERROR.to_errno();
        }

        self.inner_queue.lock();

        if self.inner_queue.item_in_queue > 0 {
            self.inner_queue.try_dequeue_stub();
            self.inner_queue.unlock();
        } else {
            if time_out == 0 {
                self.inner_queue.unlock();
                if mutex.unlock() != code::EOK.to_errno() {
                    return code::ERROR.to_errno();
                }
                return code::ETIMEDOUT.to_errno();
            } else {
                crate::debug_in_thread_context!();

                let thread = unsafe { &mut (*thread_ptr) };

                thread.error = code::EOK;
                self.inner_queue
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
                    self.inner_queue.unlock();
                    return code::ERROR.to_errno();
                }

                self.inner_queue.unlock();

                Cpu::get_current_scheduler().do_task_schedule();

                if thread.error != code::EOK {
                    result = thread.error.to_errno();
                }
            }
        }
        if mutex.lock() != code::EOK.to_errno() {
            return code::ERROR.to_errno();
        }
        return result;
    }

    #[inline] pub fn try_wait(&mut self) -> i32 {
        self.inner_queue.try_dequeue_stub()
    }

    #[inline]
    pub fn notify(&mut self) -> i32 {
        self.inner_queue.wake_dequeue_stub_sched()
    }

    #[inline]
    pub fn notify_all(&mut self) -> i32 {
        #[allow(unused_assignments)]
        let mut result = code::EOK.to_errno();
        loop {
            result = self.inner_queue.try_dequeue_stub();
            if result == code::ERROR.to_errno() {
                self.inner_queue.wake_dequeue_stub_sched();
            } else if result == code::EOK.to_errno() {
                break;
            } else {
                return code::EINVAL.to_errno();
            }
        }

        code::EOK.to_errno()
    }
}

/// bindgen for CondVar
#[allow(improper_ctypes_definitions)]
#[no_mangle]
pub extern "C" fn bindgen_condvar(_condvar: CondVar) {
    0;
}
