use crate::cpu::Cpu;
use crate::error::code;
use crate::impl_kobject;
use crate::object::{KObjectBase, KernelObject, ObjectClassType, NAME_MAX};
use crate::rt_bindings::{
    rt_debug_in_thread_context, rt_debug_not_in_interrupt, RT_EINVAL, RT_EOK, RT_ERROR,
    RT_ETIMEOUT, RT_IPC_FLAG_FIFO, RT_IPC_FLAG_PRIO, RT_TIMER_CTRL_SET_TIME, RT_UNINTERRUPTIBLE,
    RT_WAITING_FOREVER,
};
use crate::sync::lock::mutex::RtMutex;
use crate::sync::semaphore::RtSemaphore;
use crate::sync::RawSpin;
use blue_infra::list::doubly_linked_list::ListHead;
use core::ffi::c_void;
use core::ptr::null_mut;
use pinned_init::*;

/// Condition variable raw structure
#[repr(C)]
#[pin_data]
pub(crate) struct RtCondVar {
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
            (waiting_mode == RT_IPC_FLAG_FIFO as u8) || (waiting_mode == RT_IPC_FLAG_PRIO as u8)
        );

        rt_debug_not_in_interrupt!();

        pin_init!(Self {
            parent<-KObjectBase::new(ObjectClassType::ObjectClassCondVar as u8, name),
            spinlock: RawSpin::new(),
            inner_sem<-RtSemaphore::new(name, 0, waiting_mode as u8)
        })
    }

    #[inline]
    pub(crate) fn init(&mut self, name: *const i8, waiting_mode: u8) {
        assert!(
            (waiting_mode == RT_IPC_FLAG_FIFO as u8) || (waiting_mode == RT_IPC_FLAG_PRIO as u8)
        );
        self.parent
            .init(ObjectClassType::ObjectClassCondVar as u8, name);
        self.spinlock = RawSpin::new();
        self.inner_sem.init_dyn(name, 0, waiting_mode as u8);
    }

    #[inline]
    pub(crate) fn init_dyn(&mut self, name: *const i8, waiting_mode: u8) {
        assert!(
            (waiting_mode == RT_IPC_FLAG_FIFO as u8) || (waiting_mode == RT_IPC_FLAG_PRIO as u8)
        );
        self.parent
            .init_dyn(ObjectClassType::ObjectClassCondVar as u8, name);
        self.spinlock = RawSpin::new();
        self.inner_sem.init_dyn(name, 0, waiting_mode as u8);
    }

    #[inline]
    pub(crate) fn detach(&mut self) {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassCondVar as u8);

        self.inner_sem.detach();
        if self.is_static_kobject() {
            self.parent.detach();
        }
    }
    #[inline]
    pub(crate) fn wait(&mut self, mutex: &mut RtMutex) -> i32 {
        self.wait_timeout(mutex, RT_UNINTERRUPTIBLE, RT_WAITING_FOREVER)
    }

    #[inline]
    pub(crate) fn wait_timeout(
        &mut self,
        mutex: &mut RtMutex,
        pending_mode: u32,
        timeout: i32,
    ) -> i32 {
        let mut time_out = timeout;
        let mut result = RT_EOK as i32;

        let thread_ptr = crate::current_thread_ptr!();
        if mutex.owner != thread_ptr {
            return -(RT_ERROR as i32);
        }

        self.spinlock.lock();

        if self.inner_sem.inner_queue.item_in_queue > 0 {
            self.inner_sem.inner_queue.pop_stub();
            self.spinlock.unlock();
        } else {
            if time_out == 0 {
                self.spinlock.unlock();

                return -(RT_ETIMEOUT as i32);
            } else {
                rt_debug_in_thread_context!();

                let thread = unsafe { &mut (*thread_ptr) };

                thread.error = code::EOK;
                self.inner_sem
                    .inner_queue
                    .dequeue_waiter
                    .wait(thread, pending_mode);

                if time_out > 0 {
                    thread.thread_timer.timer_control(
                        RT_TIMER_CTRL_SET_TIME,
                        (&mut time_out) as *mut i32 as *mut c_void,
                    );

                    thread.thread_timer.start();
                }

                if mutex.unlock() != RT_EOK as i32 {
                    return -(RT_ERROR as i32);
                }

                self.spinlock.unlock();

                Cpu::get_current_scheduler().do_task_schedule();

                if thread.error != code::EOK {
                    result = thread.error.to_errno();
                }

                mutex.lock();
            }
        }

        return result;
    }

    #[inline]
    pub(crate) fn notify(&mut self) -> i32 {
        self.spinlock.lock();
        if !self.inner_sem.inner_queue.dequeue_waiter.is_empty() {
            self.spinlock.unlock();
            self.inner_sem.release();
        }

        self.spinlock.unlock();
        RT_EOK as i32
    }

    #[inline]
    pub(crate) fn notify_all(&mut self) -> i32 {
        let mut result = RT_EOK as i32;
        loop {
            result = self.inner_sem.try_take();
            if result == -(RT_ETIMEOUT as i32) {
                self.inner_sem.release();
            } else if result == RT_EOK as i32 {
                self.inner_sem.release();
                break;
            } else {
                return RT_EINVAL as i32;
            }
        }

        RT_EOK as i32
    }
}
