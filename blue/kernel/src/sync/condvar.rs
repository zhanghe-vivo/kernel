use crate::impl_kobject;
use crate::object::{KObjectBase, KernelObject, ObjectClassType, NAME_MAX};
use crate::rt_bindings::{
    rt_debug_not_in_interrupt, RT_EINVAL, RT_EOK, RT_ETIMEOUT, RT_IPC_FLAG_FIFO, RT_IPC_FLAG_PRIO,
};
use crate::sync::lock::mutex::RtMutex;
use crate::sync::semaphore::RtSemaphore;
use crate::sync::RawSpin;
use blue_infra::list::doubly_linked_list::ListHead;
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
    /// Wait Queue
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
    }

    #[inline]
    pub(crate) fn wait(&self, mutex: &mut RtMutex, pending_mode: u32) {}

    #[inline]
    pub(crate) fn wait_timeout(&self, timeout: u32, mutex: &RtMutex, pending_mode: u32) {}

    #[inline]
    pub(crate) fn notify(&mut self) -> i32 {
        self.spinlock.lock();
        if !self.inner_sem.inner_queue.receiver.is_empty() {
            self.spinlock.unlock();
            self.inner_sem.release();
        }

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
