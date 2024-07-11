use core::{
    cell::UnsafeCell, mem::MaybeUninit, ffi::{c_char, c_uint, c_void},
    marker::PhantomPinned,
    ptr::null_mut,
};
use crate::{rt_bindings::*,
            object::{*, rt_object_get_type, rt_object_allocate, rt_object_delete, rt_object_is_systemobject,
                     rt_object_detach, rt_object_init
                    },
            sync::ipc_common::*, error::Error,
            rt_debug_not_in_interrupt
            };
use kernel::{rt_bindings::{rt_hw_interrupt_disable, rt_hw_interrupt_enable,rt_int16_t}, rt_debug_scheduler_available, rt_object_hook_call};

use pinned_init::*;

#[cfg(feature = "RT_USING_SEMAPHORE")]
#[no_mangle]
pub extern "C" fn rt_sem_init(sem: *mut rt_semaphore, name: *const core::ffi::c_char, value: rt_uint32_t, flag: rt_uint8_t) -> rt_err_t{
    unsafe
    {
        assert!(sem != null_mut());
        assert!(value < 0x10000 as rt_uint32_t);
        assert!((flag == RT_IPC_FLAG_FIFO as rt_uint8_t) || (flag == RT_IPC_FLAG_PRIO as rt_uint8_t));
        rt_object_init(&mut ((*sem).parent.parent), RtObjectInfoType::RTObjectInfoSemaphore as u32, name);

        _rt_ipc_object_init(&mut ((*sem).parent));

        (*sem).value = value as rt_uint16_t;
        (*sem).parent.parent.flag = flag;
    }

    RT_EOK as rt_err_t
}
#[cfg(feature = "RT_USING_SEMAPHORE")]
#[no_mangle]
pub extern "C" fn rt_sem_detach(sem: *mut rt_semaphore) -> rt_err_t{
    unsafe
    {
        assert!(sem != null_mut());
        assert!(rt_object_get_type(&mut (*sem).parent.parent) == RtObjectInfoType::RTObjectInfoSemaphore as u8);
        assert!(rt_object_is_systemobject(&mut (*sem).parent.parent) == RT_TRUE as i32);

        _rt_ipc_list_resume_all(&mut ((*sem).parent.suspend_thread));

        rt_object_detach(&mut ((*sem).parent.parent));
    }

    RT_EOK as rt_err_t
}
#[cfg(all(feature = "RT_USING_SEMAPHORE", feature = "RT_USING_HEAP"))]
#[no_mangle]
pub extern "C" fn rt_sem_create(name: *const core::ffi::c_char, value: rt_uint32_t, flag : rt_uint8_t) -> rt_sem_t{
    unsafe
    {
        assert!(value < 0x10000 as rt_uint32_t);
        assert!((flag == RT_IPC_FLAG_FIFO as rt_uint8_t) || (flag == RT_IPC_FLAG_PRIO as rt_uint8_t));
        rt_debug_not_in_interrupt!();
        let sem = rt_object_allocate(RtObjectInfoType::RTObjectInfoSemaphore as u32, name) as rt_sem_t;
        if sem == null_mut() {
            return sem;
        }

        _rt_ipc_object_init(&mut (*sem).parent);

        (*sem).value = value as u16;
        (*sem).parent.parent.flag = flag;

        sem
    }
}

#[cfg(all(feature = "RT_USING_SEMAPHORE", feature = "RT_USING_HEAP"))]
#[no_mangle]
pub extern "C" fn rt_sem_delete(sem: *mut rt_semaphore) -> rt_err_t{
    unsafe
    {
        assert!(sem != null_mut());
        assert!(rt_object_get_type(&mut (*sem).parent.parent) == RtObjectInfoType::RTObjectInfoSemaphore as u8);
        assert!(rt_object_is_systemobject(&mut (*sem).parent.parent) == RT_FALSE as i32);

        rt_debug_not_in_interrupt!();

        _rt_ipc_list_resume_all(&mut ((*sem).parent.suspend_thread));

        rt_object_delete(&mut ((*sem).parent.parent));

        RT_EOK as rt_err_t
    }
}

#[cfg(feature = "RT_USING_SEMAPHORE")]
#[no_mangle]
extern "C" fn _rt_sem_take(sem : rt_sem_t, timeout : rt_int32_t, suspend_flag: i32) -> rt_err_t
{
    unsafe{
        let mut time_out = timeout as i32;
        assert!(sem != null_mut());
        assert!(rt_object_get_type(&mut (*sem).parent.parent) == RtObjectInfoType::RTObjectInfoSemaphore as u8);

        rt_object_hook_call!(rt_object_trytake_hook, (&mut ((*sem).parent.parent)));
        let check = (*sem).value == 0 && timeout != 0;
        rt_debug_scheduler_available!(check);

        let level = rt_hw_interrupt_disable();

        if (*sem).value > 0 {
            (*sem).value -= 1;
            rt_hw_interrupt_enable(level);
        } else {
            if timeout == 0
            {
                rt_hw_interrupt_enable(level);

                return -(RT_ETIMEOUT as rt_err_t);
            }
            else {
                let thread = rt_thread_self();

                (*thread).error = -(RT_EINTR as rt_err_t);

                let ret = _rt_ipc_list_suspend(&mut ((*sem).parent.suspend_thread),
                thread,
                (*sem).parent.parent.flag,
                suspend_flag);

                if ret != RT_EOK as rt_err_t {
                    rt_hw_interrupt_enable(level);
                    return ret;
                }

                if timeout > 0 {
                    rt_timer_control(&mut ((*thread).thread_timer),
                    RT_TIMER_CTRL_SET_TIME as i32,
                    (&mut time_out) as *mut i32 as *mut c_void);
                    rt_timer_start(&mut ((*thread).thread_timer));
                }

                rt_hw_interrupt_enable(level);

                rt_schedule();

                if (*thread).error != RT_EOK as rt_err_t {
                    return (*thread).error;
                }
            }
        }

        rt_object_hook_call!(rt_object_take_hook, (&mut ((*sem).parent.parent)));

        RT_EOK as rt_err_t
    }
}

#[cfg(feature = "RT_USING_SEMAPHORE")]
#[no_mangle]
pub extern "C" fn rt_sem_take(sem : rt_sem_t, time: rt_int32_t) -> rt_err_t {
    _rt_sem_take(sem, time, RT_UNINTERRUPTIBLE as i32)
}

#[cfg(feature = "RT_USING_SEMAPHORE")]
#[no_mangle]
pub extern "C" fn rt_sem_take_interruptible(sem: rt_sem_t , time: rt_int32_t) -> rt_err_t {
    _rt_sem_take(sem, time, RT_INTERRUPTIBLE as i32)
}

#[cfg(feature = "RT_USING_SEMAPHORE")]
#[no_mangle]
pub extern "C" fn rt_sem_take_killable(sem: rt_sem_t, time: rt_int32_t) -> rt_err_t {
    _rt_sem_take(sem, time, RT_KILLABLE as i32)
}

#[cfg(feature = "RT_USING_SEMAPHORE")]
#[no_mangle]
pub extern "C" fn rt_sem_trytake(sem: rt_sem_t) -> rt_err_t {
    rt_sem_take(sem, RT_WAITING_NO as i32)
}

#[cfg(feature = "RT_USING_SEMAPHORE")]
#[no_mangle]
pub extern "C" fn rt_sem_release(sem : rt_sem_t ) -> rt_err_t {
    unsafe{
        assert!(sem != null_mut());
        assert!(rt_object_get_type(&mut (*sem).parent.parent) == RtObjectInfoType::RTObjectInfoSemaphore as u8);
        rt_object_hook_call!(rt_object_put_hook, (&mut ((*sem).parent.parent)));

        let mut need_schedule = RT_FALSE;
        let level = rt_hw_interrupt_disable();

        if (*sem).parent.suspend_thread.is_empty() {
            _rt_ipc_list_resume(&mut ((*sem).parent.suspend_thread));
            need_schedule = RT_TRUE;
        } else {
            if (*sem).value < RT_SEM_VALUE_MAX as rt_uint16_t {
                (*sem).value += 1;
            } else {
                rt_hw_interrupt_enable(level);
                return -(RT_EFULL as rt_err_t);
            }
        }

        rt_hw_interrupt_enable(level);

        if need_schedule == RT_TRUE {
            rt_schedule();
        }

        RT_EOK as rt_err_t
    }
}

#[cfg(feature = "RT_USING_SEMAPHORE")]
#[no_mangle]
pub extern "C" fn rt_sem_control(sem : rt_sem_t , cmd : i32, arg: *const c_void) -> rt_err_t
{
    unsafe {
        assert!(sem != null_mut());
        assert!(rt_object_get_type(&mut (*sem).parent.parent) == RtObjectInfoType::RTObjectInfoSemaphore as u8);

        if cmd == RT_IPC_CMD_RESET as i32 {
            let value = arg as rt_ubase_t;
            let level = rt_hw_interrupt_disable();

            _rt_ipc_list_resume_all(&mut (*sem).parent.suspend_thread);

            (*sem).value = value as rt_uint16_t;

            rt_hw_interrupt_enable(level);

            rt_schedule();

            return RT_EOK as rt_err_t;
        }

        -(RT_ERROR as rt_err_t)
    }
}

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
        let mut result = unsafe { rt_sem_create(name.as_ptr() as *const c_char, value, RT_IPC_FLAG_PRIO as u8) };
        if result == RT_NULL as rt_sem_t{
            Err(Error::from_errno(RT_ERROR as i32))
        } else {
            Ok( Self {sem_ptr: result, _pin: PhantomPinned {} })
        }
    }

    pub fn delete(self) -> Result<(), Error> {
        let result = unsafe { rt_sem_delete(self.sem_ptr) };
        if result == RT_EOK as rt_err_t {
            Ok(())
        } else {
            Err(Error::from_errno(result as i32))
        }
    }

    pub fn acquire(&self) {
        let _result = unsafe { rt_sem_take(self.sem_ptr, RT_WAITING_FOREVER) };
    }

    pub fn acquire_wait(&self, tick: i32) {
        let _result = unsafe { rt_sem_take(self.sem_ptr, tick) };
    }

    pub fn acquire_no_wait(&self) {
        let _result = unsafe { rt_sem_trytake(self.sem_ptr) };
    }

    pub fn release(&self) {
        let _result = unsafe { rt_sem_release(self.sem_ptr) };
    }

    pub fn access(&self) -> SemaphoreGuard {
        self.acquire();
        SemaphoreGuard { _sem: self }
    }
}

#[pin_data]
pub struct SemaphoreStatic {
    #[pin]
    sem_: UnsafeCell<MaybeUninit<rt_semaphore>>,
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
        let result = unsafe { rt_sem_init(self.sem_.get().cast(), name.as_ptr() as *const c_char, value, RT_IPC_FLAG_PRIO as u8) };
        if result == RT_EOK as i32{
            Ok(())
        } else {
            Err(Error::from_errno(result))
        }
    }

    pub fn detach(&'static self) -> Result<(), Error> {
        let result = unsafe { rt_sem_detach(self.sem_.get().cast()) };
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