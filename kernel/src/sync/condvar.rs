use crate::{
    clock::WAITING_FOREVER,
    cpu::Cpu,
    error::{code, Error},
    impl_kobject,
    object::{KObjectBase, KernelObject, ObjectClassType, NAME_MAX},
    sync::{ipc_common::IPC_SYS_QUEUE_STUB, lock::mutex::Mutex, wait_list::WaitMode},
    thread::SuspendFlag,
    timer::TimerControlAction,
};
use core::{ffi::c_void, ptr::null_mut};
use pinned_init::{pin_data, pin_init, PinInit};

use super::ipc_common::SysQueue;

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
    pub(crate) fn new(name: [i8; NAME_MAX], waiting_mode: WaitMode) -> impl PinInit<Self> {
        crate::debug_not_in_interrupt!();

        pin_init!(Self {
            parent<-KObjectBase::new(ObjectClassType::ObjectClassCondVar as u8, name),
            inner_queue<-SysQueue::new(
                core::mem::size_of::<u32>(),
                0,
                IPC_SYS_QUEUE_STUB,
                waiting_mode,
            )
        })
    }

    #[allow(dead_code)]
    #[inline]
    pub fn init(&mut self, name: *const i8, waiting_mode: WaitMode) {
        self.parent
            .init(ObjectClassType::ObjectClassCondVar as u8, name);
        self.inner_queue.init(
            null_mut(),
            core::mem::size_of::<u32>(),
            0,
            IPC_SYS_QUEUE_STUB,
            waiting_mode,
        );
    }

    #[inline]
    pub(crate) fn init_dyn(&mut self, name: *const i8, waiting_mode: WaitMode) {
        self.parent
            .init_dyn(ObjectClassType::ObjectClassCondVar as u8, name);
        self.inner_queue.init(
            null_mut(),
            core::mem::size_of::<u32>(),
            0,
            IPC_SYS_QUEUE_STUB,
            waiting_mode,
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
    pub fn wait(&mut self, mutex: &mut Mutex) -> Result<(), Error> {
        self.wait_timeout(mutex, SuspendFlag::Uninterruptible, WAITING_FOREVER as i32)
    }

    #[inline]
    pub(crate) fn wait_timeout(
        &mut self,
        mutex: &mut Mutex,
        pending_mode: SuspendFlag,
        timeout: i32,
    ) -> Result<(), Error> {
        let mut time_out = timeout;

        let thread_ptr = crate::current_thread_ptr!();
        if mutex.owner != thread_ptr {
            return Err(code::ERROR);
        }

        self.inner_queue.lock();

        if self.inner_queue.item_in_queue > 0 {
            let _ = self.inner_queue.try_dequeue_stub();
            self.inner_queue.unlock();
        } else {
            if time_out == 0 {
                mutex.unlock()?;
                return Err(code::ETIMEDOUT);
            } else {
                crate::debug_in_thread_context!();
                let thread = unsafe { &mut (*thread_ptr) };
                thread.error = code::EOK;

                self.inner_queue
                    .dequeue_waiter
                    .wait(thread, pending_mode)
                    .map_err(|e| {
                        self.inner_queue.unlock();
                        e
                    })?;

                if time_out > 0 {
                    thread.thread_timer.timer_control(
                        TimerControlAction::SetTime,
                        (&mut time_out) as *mut i32 as *mut c_void,
                    );

                    thread.thread_timer.start();
                }

                self.inner_queue.unlock();
                mutex.unlock()?;

                Cpu::get_current_scheduler().do_task_schedule();

                if thread.error != code::EOK {
                    return Err(thread.error);
                }
            }
        }

        // hold mutex again
        mutex.lock()?;
        Ok(())
    }

    #[inline]
    pub fn try_wait(&mut self) -> Result<(), Error> {
        self.inner_queue.try_dequeue_stub()
    }

    #[inline]
    pub fn notify(&mut self) -> Result<(), Error> {
        self.inner_queue.wake_dequeue_stub_sched()
    }

    #[inline]
    pub fn notify_all(&mut self) -> Result<(), Error> {
        loop {
            match self.inner_queue.try_dequeue_stub() {
                Ok(()) => break,
                Err(code::EBUSY) => {
                    let _ = self.inner_queue.wake_dequeue_stub_sched();
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }

        Ok(())
    }
}

/// bindgen for CondVar
#[allow(improper_ctypes_definitions)]
#[no_mangle]
pub extern "C" fn bindgen_condvar(_condvar: CondVar) {
    0;
}
