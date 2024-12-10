use crate::{
    impl_kobject,
    object::{KObjectBase, KernelObject, ObjectClassType, NAME_MAX},
    rt_bindings::{
        rt_debug_not_in_interrupt, RT_EBUSY, RT_EOK, RT_IPC_FLAG_FIFO, RT_IPC_FLAG_PRIO,
        RT_UNINTERRUPTIBLE,
    },
    sync::{condvar::RtCondVar, lock::mutex::RtMutex},
};
use blue_infra::list::doubly_linked_list::ListHead;
use pinned_init::{pin_data, pin_init, PinInit};

/// RwLock Raw Structure
#[repr(C)]
#[pin_data]
pub struct RtRwLock {
    // kernel object
    #[pin]
    parent: KObjectBase,
    /// Mutex that inner used for condvar
    #[pin]
    mutex: RtMutex,
    /// Condition var for reader notification
    #[pin]
    read_cond: RtCondVar,
    /// Condition var for writer notification
    #[pin]
    write_cond: RtCondVar,
    /// Lock flag, which indicates >0 for readers occupied count, -1 for writer occupy
    rw_count: i32,
    /// Readers wait for this condition var
    reader_waiting: u32,
    /// Writers ait for this condition var
    writer_waiting: u32,
}

impl_kobject!(RtRwLock);

impl RtRwLock {
    #[inline]
    pub(crate) fn new(name: [i8; NAME_MAX], waiting_mode: u8) -> impl PinInit<Self> {
        assert!(
            (waiting_mode == RT_IPC_FLAG_FIFO as u8) || (waiting_mode == RT_IPC_FLAG_PRIO as u8)
        );

        rt_debug_not_in_interrupt!();

        pin_init!(Self {
            parent<-KObjectBase::new(ObjectClassType::ObjectClassRwLock as u8, name),
            mutex<-RtMutex::new(name, waiting_mode as u8),
            read_cond<-RtCondVar::new(name, waiting_mode as u8),
            write_cond<-RtCondVar::new(name, waiting_mode as u8),
            rw_count:0,
            reader_waiting:0,
            writer_waiting:0,
        })
    }

    #[inline]
    fn init_internal(&mut self, name: *const i8, waiting_mode: u8) {
        self.mutex.init_dyn(name, waiting_mode as u8);
        self.read_cond.init_dyn(name, waiting_mode as u8);
        self.write_cond.init_dyn(name, waiting_mode as u8);
        self.rw_count = 0;
        self.reader_waiting = 0;
        self.writer_waiting = 0;
    }
    #[inline]
    pub(crate) fn init(&mut self, name: *const i8, waiting_mode: u8) {
        assert!(
            (waiting_mode == RT_IPC_FLAG_FIFO as u8) || (waiting_mode == RT_IPC_FLAG_PRIO as u8)
        );
        self.parent
            .init(ObjectClassType::ObjectClassRwLock as u8, name);
        self.init_internal(name, waiting_mode);
    }

    #[inline]
    pub(crate) fn init_dyn(&mut self, name: *const i8, waiting_mode: u8) {
        assert!(
            (waiting_mode == RT_IPC_FLAG_FIFO as u8) || (waiting_mode == RT_IPC_FLAG_PRIO as u8)
        );
        self.parent
            .init_dyn(ObjectClassType::ObjectClassRwLock as u8, name);
        self.init_internal(name, waiting_mode);
    }

    #[inline]
    pub(crate) fn detach(&mut self) -> i32 {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassRwLock as u8);

        let mut result = self.mutex.lock();
        if result != RT_EOK as i32 {
            return result;
        }

        if self.rw_count != 0 || self.reader_waiting != 0 || self.writer_waiting != 0 {
            return -(RT_EBUSY as i32);
        } else {
            result = self.read_cond.inner_sem.try_take();
            if result == RT_EOK as i32 {
                result = self.write_cond.inner_sem.try_take();
                if result == RT_EOK as i32 {
                    self.read_cond.inner_sem.release();
                    self.write_cond.inner_sem.release();
                    self.read_cond.detach();
                    self.write_cond.detach();
                } else {
                    self.read_cond.detach();
                    result = -(RT_EBUSY as i32);
                }
            } else {
                result = -(RT_EBUSY as i32);
            }
        }

        self.mutex.unlock();
        if result == RT_EOK as i32 {
            self.mutex.detach();
        }

        return result;
    }
    pub(crate) fn lock_read(&mut self) -> i32 {
        let mut result = self.mutex.lock();
        if result != RT_EOK as i32 {
            return result;
        }

        while self.rw_count < 0 || self.writer_waiting > 0 {
            self.reader_waiting += 1;
            result = self.read_cond.wait(&mut self.mutex);
            self.reader_waiting -= 1;
            if result != RT_EOK as i32 {
                break;
            }
        }

        if result == RT_EOK as i32 {
            self.rw_count += 1;
        }

        self.mutex.unlock();

        result
    }

    pub(crate) fn try_lock_read(&mut self) -> i32 {
        let mut result = self.mutex.lock();

        if result != RT_EOK as i32 {
            return result;
        }

        if self.rw_count < 0 || self.writer_waiting > 0 {
            result = -(RT_EBUSY as i32);
        } else {
            self.rw_count += 1;
        }

        self.mutex.unlock();

        return result;
    }

    pub(crate) fn lock_read_wait(&mut self, timeout: i32) -> i32 {
        let mut result = self.mutex.lock();

        if result != RT_EOK as i32 {
            return result;
        }

        while self.rw_count < 0 || self.writer_waiting > 0 {
            self.reader_waiting += 1;
            result = self
                .read_cond
                .wait_timeout(&mut self.mutex, RT_UNINTERRUPTIBLE, timeout);
            self.reader_waiting -= 1;
            if result != RT_EOK as i32 {
                break;
            }
        }

        if result == RT_EOK as i32 {
            self.rw_count += 1;
        }

        self.mutex.unlock();

        result
    }

    pub(crate) fn lock_write_wait(&mut self, timeout: i32) -> i32 {
        let mut result = self.mutex.lock();

        if result != RT_EOK as i32 {
            return result;
        }

        while self.rw_count != 0 {
            self.writer_waiting += 1;
            result = self
                .write_cond
                .wait_timeout(&mut self.mutex, RT_UNINTERRUPTIBLE, timeout);
            self.writer_waiting -= 1;

            if result != RT_EOK as i32 {
                break;
            }
        }

        if result == RT_EOK as i32 {
            self.rw_count = -1;
        }

        self.mutex.unlock();

        result
    }

    pub(crate) fn lock_write(&mut self) -> i32 {
        let mut result = self.mutex.lock();

        if result != RT_EOK as i32 {
            return result;
        }

        while self.rw_count != 0 {
            self.writer_waiting += 1;
            result = self.write_cond.wait(&mut self.mutex);
            self.writer_waiting -= 1;

            if result != RT_EOK as i32 {
                break;
            }
        }

        if result == RT_EOK as i32 {
            self.rw_count = -1;
        }

        self.mutex.unlock();

        result
    }

    pub(crate) fn try_lock_write(&mut self) -> i32 {
        let mut result = self.mutex.lock();

        if result != RT_EOK as i32 {
            return result;
        }

        if self.rw_count != 0 {
            result = -(RT_EBUSY as i32);
        } else {
            self.rw_count = -1;
        }

        self.mutex.unlock();

        return result;
    }

    pub(crate) fn unlock(&mut self) -> i32 {
        let mut result = self.mutex.lock();

        if result != RT_EOK as i32 {
            return result;
        }

        if self.rw_count > 0 {
            self.rw_count -= 1;
        } else if self.rw_count == -1 {
            self.rw_count = 0;
        }

        if self.writer_waiting > 0 {
            if self.rw_count == 0 {
                result = self.write_cond.notify();
            }
        } else if self.reader_waiting > 0 {
            result = self.read_cond.notify_all();
        }

        self.mutex.unlock();

        result
    }
}
