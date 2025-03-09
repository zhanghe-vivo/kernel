use crate::{
    error::{code, Error},
    impl_kobject,
    object::{KObjectBase, KernelObject, ObjectClassType, NAME_MAX},
    sync::{condvar::CondVar, lock::mutex::Mutex, wait_list::WaitMode},
    thread::SuspendFlag,
};
use pinned_init::{pin_data, pin_init, PinInit};

/// RwLock Raw Structure
#[repr(C)]
#[pin_data]
pub struct RwLock {
    // kernel object
    #[pin]
    parent: KObjectBase,
    /// Mutex that inner used for rwlock
    #[pin]
    mutex: Mutex,
    /// Condition var for reader notification
    #[pin]
    read_cond: CondVar,
    /// Condition var for writer notification
    #[pin]
    write_cond: CondVar,
    /// Lock flag, which indicates >0 for readers occupied count, -1 for writer occupy
    rw_count: i32,
    /// Readers wait for this condition var
    reader_waiting: u32,
    /// Writers ait for this condition var
    writer_waiting: u32,
}

impl_kobject!(RwLock);

impl RwLock {
    #[inline]
    pub(crate) fn new(name: [i8; NAME_MAX], waiting_mode: WaitMode) -> impl PinInit<Self> {
        crate::debug_not_in_interrupt!();

        pin_init!(Self {
            parent<-KObjectBase::new(ObjectClassType::ObjectClassRwLock, name),
            mutex<-Mutex::new(name),
            read_cond<-CondVar::new(name, waiting_mode),
            write_cond<-CondVar::new(name, waiting_mode),
            rw_count:0,
            reader_waiting:0,
            writer_waiting:0,
        })
    }

    #[inline]
    fn init_internal(&mut self, name: *const i8, waiting_mode: WaitMode) {
        self.mutex.init_dyn(name);
        self.read_cond.init_dyn(name, waiting_mode);
        self.write_cond.init_dyn(name, waiting_mode);
        self.rw_count = 0;
        self.reader_waiting = 0;
        self.writer_waiting = 0;
    }
    #[inline]
    pub fn init(&mut self, name: *const i8, waiting_mode: WaitMode) {
        self.parent.init(ObjectClassType::ObjectClassRwLock, name);
        self.init_internal(name, waiting_mode);
    }

    #[inline]
    pub(crate) fn init_dyn(&mut self, name: *const i8, waiting_mode: WaitMode) {
        self.parent
            .init_dyn(ObjectClassType::ObjectClassRwLock, name);
        self.init_internal(name, waiting_mode);
    }

    #[inline]
    pub fn detach(&mut self) -> Result<(), Error> {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassRwLock);

        let guard = self.mutex.acquire()?;

        if self.rw_count != 0 || self.reader_waiting != 0 || self.writer_waiting != 0 {
            return Err(code::EBUSY);
        } else {
            let _ = self.read_cond.try_wait().map_or((), |_| {
                let _ = self.read_cond.notify_all();
                self.read_cond.detach();
            });

            let _ = self.write_cond.try_wait().map_or((), |_| {
                let _ = self.write_cond.notify_all();
                self.write_cond.detach();
            });
        }

        drop(guard);
        self.mutex.detach();

        Ok(())
    }
    pub fn lock_read(&mut self) -> Result<(), Error> {
        self.mutex.lock()?;
        while self.rw_count < 0 || self.writer_waiting > 0 {
            self.reader_waiting += 1;
            match self.read_cond.wait(&mut self.mutex) {
                Ok(()) => {
                    self.reader_waiting -= 1;
                }
                Err(_) => {
                    self.reader_waiting -= 1;
                    return self.mutex.unlock();
                }
            }
        }

        self.rw_count += 1;
        self.mutex.unlock()?;
        Ok(())
    }

    pub fn try_lock_read(&mut self) -> Result<(), Error> {
        let _ = self.mutex.acquire()?;

        if self.rw_count < 0 || self.writer_waiting > 0 {
            return Err(code::EBUSY);
        } else {
            self.rw_count += 1;
        }
        Ok(())
    }

    pub(crate) fn lock_read_wait(&mut self, timeout: i32) -> Result<(), Error> {
        self.mutex.lock()?;

        while self.rw_count < 0 || self.writer_waiting > 0 {
            self.reader_waiting += 1;
            match self.read_cond.wait_timeout(
                &mut self.mutex,
                SuspendFlag::Uninterruptible,
                timeout,
            ) {
                Ok(()) => {
                    self.reader_waiting -= 1;
                }
                Err(_) => {
                    self.reader_waiting -= 1;
                    return self.mutex.unlock();
                }
            }
        }
        self.rw_count += 1;
        self.mutex.unlock()?;
        Ok(())
    }

    pub(crate) fn lock_write_wait(&mut self, timeout: i32) -> Result<(), Error> {
        self.mutex.lock()?;

        while self.rw_count != 0 {
            self.writer_waiting += 1;
            match self.write_cond.wait_timeout(
                &mut self.mutex,
                SuspendFlag::Uninterruptible,
                timeout,
            ) {
                Ok(()) => {
                    self.writer_waiting -= 1;
                }
                Err(_) => {
                    self.writer_waiting -= 1;
                    return self.mutex.unlock();
                }
            }
        }
        self.rw_count = -1;
        self.mutex.unlock()?;
        Ok(())
    }

    pub fn lock_write(&mut self) -> Result<(), Error> {
        self.mutex.lock()?;

        while self.rw_count != 0 {
            self.writer_waiting += 1;
            match self.write_cond.wait(&mut self.mutex) {
                Ok(()) => {
                    self.writer_waiting -= 1;
                }
                Err(_) => {
                    self.writer_waiting -= 1;
                    return self.mutex.unlock();
                }
            }
        }

        self.rw_count = -1;
        self.mutex.unlock()?;
        Ok(())
    }

    pub fn try_lock_write(&mut self) -> Result<(), Error> {
        let _ = self.mutex.acquire()?;

        if self.rw_count != 0 {
            return Err(code::EBUSY);
        } else {
            self.rw_count = -1;
        }

        Ok(())
    }

    pub fn unlock(&mut self) -> Result<(), Error> {
        let _ = self.mutex.acquire()?;

        if self.rw_count > 0 {
            self.rw_count -= 1;
        } else if self.rw_count == -1 {
            self.rw_count = 0;
        }

        if self.writer_waiting > 0 {
            if self.rw_count == 0 {
                self.write_cond.notify()?;
            }
        } else if self.reader_waiting > 0 {
            self.read_cond.notify_all()?;
        }

        Ok(())
    }
}

/// bindgen for RwLock
#[allow(improper_ctypes_definitions)]
#[no_mangle]
pub extern "C" fn bindgen_rwlock(_rwlock: RwLock) {
    0;
}
