use crate::{
    rt_bindings::{self, RT_EOK, rt_sem_t, RT_WAITING_FOREVER},
    error::Error,
};

use core::{
    cell::UnsafeCell, mem::MaybeUninit, ffi::c_char,
    marker::PhantomPinned,
};

use pinned_init::*;

pub struct SemaphoreGuard<'a> {
    _sem: &'a Semaphore,
}

impl<'a> Drop for SemaphoreGuard<'a> {
    fn drop(&mut self) {
        self._sem.release();
    }
}

#[pin_data]
pub struct Semaphore {
    #[pin]
    sem_ptr: rt_sem_t,
    #[pin]
    _pin: PhantomPinned,
}

unsafe impl Send for Semaphore {}
unsafe impl Sync for Semaphore {}

impl Semaphore {
    pub fn new(name: &str, value: u32) -> Result<Self, Error> {
        let result = unsafe { rt_bindings::rt_sem_create(name.as_ptr() as *const c_char, value, rt_bindings::RT_IPC_FLAG_PRIO as u8) };
        if result == rt_bindings::RT_NULL as rt_sem_t{
            Err(Error::from_errno(rt_bindings::RT_ERROR as i32))
        } else {
            Ok( Self {sem_ptr: result, _pin: PhantomPinned {} })
        }
    }

    pub fn delete(self) -> Result<(), Error> {
        let result = unsafe { rt_bindings::rt_sem_delete(self.sem_ptr) };
        if result == RT_EOK as i32 {
            Ok(())
        } else {
            Err(Error::from_errno(result))
        }
    }

    pub fn acquire(&self) {
        let _result = unsafe { rt_bindings::rt_sem_take(self.sem_ptr, RT_WAITING_FOREVER) };
    }

    pub fn acquire_wait(&self, tick: i32) {
        let _result = unsafe { rt_bindings::rt_sem_take(self.sem_ptr, tick) };
    }

    pub fn acquire_no_wait(&self) {
        let _result = unsafe { rt_bindings::rt_sem_trytake(self.sem_ptr) };
    }

    pub fn release(&self) {
        let _result = unsafe { rt_bindings::rt_sem_release(self.sem_ptr) };
    }

    pub fn access(&self) -> SemaphoreGuard {
        self.acquire();
        SemaphoreGuard { _sem: self }
    }
}

#[pin_data]
pub struct SemaphoreStatic {
    #[pin]
    sem_: UnsafeCell<MaybeUninit<rt_bindings::rt_semaphore>>,
    #[pin]
    pinned_: PhantomPinned,
}

unsafe impl Send for SemaphoreStatic {}
unsafe impl Sync for SemaphoreStatic {}

impl SemaphoreStatic {
    pub const fn new() -> Self {
        SemaphoreStatic {
            sem_: UnsafeCell::new(MaybeUninit::uninit()),
            pinned_: PhantomPinned {},
        }
    }
    pub fn init(&'static self, name: &str, value: u32) -> Result<(), Error> {
        let result = unsafe { rt_bindings::rt_sem_init(self.sem_.get().cast(), name.as_ptr() as *const c_char, value, rt_bindings::RT_IPC_FLAG_PRIO as u8) };
        if result == RT_EOK as i32{
            Ok(())
        } else {
            Err(Error::from_errno(result))
        }
    }

    pub fn detach(&'static self) -> Result<(), Error> {
        let result = unsafe { rt_bindings::rt_sem_detach(self.sem_.get().cast()) };
        if result == RT_EOK as i32{
            Ok(())
        } else {
            Err(Error::from_errno(result))
        }
    }

    pub fn get(&'static self) -> Semaphore {
        Semaphore {
            sem_ptr: self.sem_.get().cast(),
            _pin: PhantomPinned {}
        }
    }
}

