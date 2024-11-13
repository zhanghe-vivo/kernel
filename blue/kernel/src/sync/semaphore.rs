use crate::{
    cpu::Cpu,
    error::Error,
    impl_kobject,
    linked_list::ListHead,
    list_head_for_each,
    object::{
        rt_object_allocate, rt_object_delete, rt_object_detach, rt_object_get_type,
        rt_object_is_systemobject, ObjectClassType, *,
    },
    print, println,
    rt_bindings::*,
    sync::ipc_common::*,
    sync::lock::spinlock::RawSpin,
    thread::RtThread,
    timer,
};
use core::pin::Pin;
use core::{
    ffi::{self, c_char, c_void},
    ptr::null_mut,
};

use crate::str::CStr;
//use crate::sync::SpinLock;
use pinned_init::*;

/// Semaphore raw structure
#[repr(C)]
#[pin_data]
pub struct RtSemaphore {
    /// Inherit from IPCObject
    #[pin]
    pub(crate) parent: IPCObject,
    /// Value of semaphore
    pub(crate) value: ffi::c_ushort,
    /// Reserved field
    pub(crate) reserved: ffi::c_ushort,
}

impl_kobject!(RtSemaphore);

impl RtSemaphore {
    #[inline]
    pub(crate) fn init(&mut self, name: *const i8, value: u16, flag: u8) {
        assert!((flag == RT_IPC_FLAG_FIFO as u8) || (flag == RT_IPC_FLAG_PRIO as u8));

        self.parent
            .init(ObjectClassType::ObjectClassSemaphore as u8, name, flag);
        self.value = value;
    }

    #[inline]
    pub fn detach(&mut self) {
        assert_eq!(
            self.type_name(),
            ObjectClassType::ObjectClassSemaphore as u8
        );
        assert!(self.is_static_kobject());

        self.parent.wake_all();
        self.parent.parent.detach();
    }

    #[inline]
    pub fn new(name: *const i8, value: u16, flag: u8) -> *mut Self {
        assert!((flag == RT_IPC_FLAG_FIFO as u8) || (flag == RT_IPC_FLAG_PRIO as u8));

        rt_debug_not_in_interrupt!();

        let sem = IPCObject::new::<Self>(ObjectClassType::ObjectClassSemaphore as u8, name, flag);
        if !sem.is_null() {
            unsafe { (*sem).value = value };
        }

        sem
    }

    #[inline]
    pub fn delete(&mut self) {
        assert_eq!(
            self.type_name(),
            ObjectClassType::ObjectClassSemaphore as u8
        );
        assert!(!self.is_static_kobject());

        rt_debug_not_in_interrupt!();

        self.parent.wake_all();
        self.parent.parent.delete();
    }

    fn take_internal(&mut self, timeout: i32, suspend_flag: u32) -> ffi::c_long {
        let mut time_out = timeout as i32;
        assert_eq!(
            self.type_name(),
            ObjectClassType::ObjectClassSemaphore as u8
        );
        // SAFETY: only one thread can take the semaphore
        unsafe {
            rt_object_hook_call!(
                rt_object_trytake_hook,
                &mut self.parent.parent as *mut KObjectBase as *mut rt_object
            );
        }

        #[allow(unused_variables)]
        let check = self.value == 0 && timeout != 0;
        rt_debug_scheduler_available!(check);

        self.parent.lock();

        if self.value > 0 {
            self.value -= 1;
            self.parent.unlock();
        } else {
            if timeout == 0 {
                self.parent.unlock();

                /* FIXME: -2 is as expected, while C -RT_ETIMEOUT is -116. */
                return -116; //(RT_ETIMEOUT as rt_err_t);
            } else {
                let thread = unsafe { crate::current_thread!().unwrap().as_mut() };

                (*thread).error = -(RT_EINTR as ffi::c_long);

                let ret = self
                    .parent
                    .wait(thread, self.parent.flag, suspend_flag as u32);

                if ret != RT_EOK as ffi::c_long {
                    self.parent.unlock();
                    return ret;
                }

                if timeout > 0 {
                    (*thread).thread_timer.timer_control(
                        RT_TIMER_CTRL_SET_TIME as u32,
                        (&mut time_out) as *mut i32 as *mut c_void,
                    );
                    (*thread).thread_timer.start();
                }

                self.parent.unlock();

                Cpu::get_current_scheduler().do_task_schedule();

                if (*thread).error != RT_EOK as ffi::c_long {
                    return (*thread).error;
                }
            }
        }
        // SAFETY: only one thread can take the semaphore
        unsafe {
            rt_object_hook_call!(
                rt_object_take_hook,
                &mut self.parent.parent as *mut KObjectBase as *mut rt_object
            );
        }

        RT_EOK as ffi::c_long
    }

    pub fn take(&mut self, timeout: i32) -> ffi::c_long {
        self.take_internal(timeout, RT_UNINTERRUPTIBLE as u32)
    }

    pub fn try_take(&mut self) -> ffi::c_long {
        self.take(0)
    }

    pub fn take_interruptible(&mut self, timeout: i32) -> ffi::c_long {
        self.take_internal(timeout, RT_INTERRUPTIBLE as u32)
    }

    pub fn take_killable(&mut self, timeout: i32) -> ffi::c_long {
        self.take_internal(timeout, RT_KILLABLE as u32)
    }

    pub fn release(&mut self) -> ffi::c_long {
        assert_eq!(
            self.type_name(),
            ObjectClassType::ObjectClassSemaphore as u8
        );
        // SAFETY: only one thread can take the semaphore
        unsafe {
            rt_object_hook_call!(
                rt_object_put_hook,
                &mut self.parent.parent as *mut KObjectBase as *mut rt_object
            );
        }

        let mut need_schedule = false;
        self.parent.lock();

        if self.parent.has_waiting() {
            self.parent.wake_one();
            need_schedule = true;
        } else {
            if self.value < RT_SEM_VALUE_MAX as u16 {
                self.value += 1;
            } else {
                self.parent.unlock();
                return -(RT_EFULL as ffi::c_long);
            }
        }

        self.parent.unlock();

        if need_schedule {
            Cpu::get_current_scheduler().do_task_schedule();
        }

        RT_EOK as ffi::c_long
    }

    pub fn control(&mut self, cmd: i32, arg: *const c_void) -> ffi::c_long {
        assert_eq!(
            self.type_name(),
            ObjectClassType::ObjectClassSemaphore as u8
        );

        if cmd == RT_IPC_CMD_RESET as i32 {
            let value = arg as ffi::c_ulong;
            self.parent.lock();

            self.parent.wake_all();

            self.value = value as rt_uint16_t;

            self.parent.unlock();

            Cpu::get_current_scheduler().do_task_schedule();

            return RT_EOK as ffi::c_long;
        }

        -(RT_ERROR as rt_err_t)
    }
}

#[cfg(feature = "RT_USING_SEMAPHORE")]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_init(
    sem: *mut RtSemaphore,
    name: *const core::ffi::c_char,
    value: rt_uint32_t,
    flag: rt_uint8_t,
) -> rt_err_t {
    assert!(!sem.is_null());
    (*sem).init(name, value as u16, flag);
    RT_EOK as rt_err_t
}
#[cfg(feature = "RT_USING_SEMAPHORE")]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_detach(sem: *mut RtSemaphore) -> rt_err_t {
    assert!(!sem.is_null());
    (*sem).detach();
    RT_EOK as rt_err_t
}
#[cfg(all(feature = "RT_USING_SEMAPHORE", feature = "RT_USING_HEAP"))]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_create(
    name: *const core::ffi::c_char,
    value: rt_uint32_t,
    flag: rt_uint8_t,
) -> *mut RtSemaphore {
    RtSemaphore::new(name, value as u16, flag)
}

#[cfg(all(feature = "RT_USING_SEMAPHORE", feature = "RT_USING_HEAP"))]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_delete(sem: *mut RtSemaphore) -> rt_err_t {
    assert!(!sem.is_null());
    (*sem).delete();
    RT_EOK as rt_err_t
}

#[cfg(feature = "RT_USING_SEMAPHORE")]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_take(sem: *mut RtSemaphore, time: rt_int32_t) -> rt_err_t {
    assert!(!sem.is_null());
    (*sem).take(time)
}

#[cfg(feature = "RT_USING_SEMAPHORE")]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_take_interruptible(
    sem: *mut RtSemaphore,
    time: rt_int32_t,
) -> rt_err_t {
    assert!(!sem.is_null());
    (*sem).take_interruptible(time)
}

#[cfg(feature = "RT_USING_SEMAPHORE")]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_take_killable(sem: *mut RtSemaphore, time: rt_int32_t) -> rt_err_t {
    assert!(!sem.is_null());
    (*sem).take_killable(time)
}

#[cfg(feature = "RT_USING_SEMAPHORE")]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_trytake(sem: *mut RtSemaphore) -> rt_err_t {
    assert!(!sem.is_null());
    (*sem).try_take()
}

#[cfg(feature = "RT_USING_SEMAPHORE")]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_release(sem: *mut RtSemaphore) -> rt_err_t {
    assert!(!sem.is_null());
    (*sem).release()
}

#[cfg(feature = "RT_USING_SEMAPHORE")]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_control(
    sem: *mut RtSemaphore,
    cmd: i32,
    arg: *const c_void,
) -> rt_err_t {
    assert!(!sem.is_null());
    (*sem).control(cmd, arg)
}

pub struct SemaphoreGuard<'a> {
    _sem: &'a Semaphore,
}
impl<'a> Drop for SemaphoreGuard<'a> {
    fn drop(&mut self) {
        self._sem.release();
    }
}

pub struct Semaphore {
    raw_sem: *mut RtSemaphore,
}

unsafe impl Send for Semaphore {}
unsafe impl Sync for Semaphore {}

impl Semaphore {
    pub fn new(name: &str, value: u32) -> Result<Self, Error> {
        let result = unsafe {
            rt_sem_create(
                name.as_ptr() as *const c_char,
                value,
                RT_IPC_FLAG_PRIO as u8,
            )
        };
        unsafe {
            if result == RT_NULL as *mut RtSemaphore {
                Err(Error::from_errno(RT_ERROR as i32))
            } else {
                Ok(Self { raw_sem: result })
            }
        }
    }

    fn delete(self) -> Result<(), Error> {
        let result = unsafe { rt_sem_delete(self.raw_sem) };
        if result == RT_EOK as rt_err_t {
            Ok(())
        } else {
            Err(Error::from_errno(result as i32))
        }
    }

    pub fn acquire(&self) {
        let _result = unsafe { rt_sem_take(self.raw_sem, RT_WAITING_FOREVER) };
    }

    pub fn acquire_wait(&self, tick: i32) {
        let _result = unsafe { rt_sem_take(self.raw_sem, tick) };
    }

    pub fn acquire_no_wait(&self) {
        let _result = unsafe { rt_sem_trytake(self.raw_sem) };
    }

    pub fn release(&self) {
        let _result = unsafe { rt_sem_release(self.raw_sem) };
    }

    pub fn access(&self) -> SemaphoreGuard {
        self.acquire();
        SemaphoreGuard { _sem: self }
    }
}

#[no_mangle]
#[allow(unused_unsafe)]
pub extern "C" fn rt_sem_info() {
    let callback_forword = || {
        println!("semaphor v   suspend thread");
        println!("-------- --- --------------");
    };
    let callback = |node: &ListHead| unsafe {
        let sem =
            &*(crate::list_head_entry!(node.as_ptr(), KObjectBase, list) as *const RtSemaphore);
        let _ = crate::format_name!(sem.parent.parent.name.as_ptr(), 8);
        print!(" {:03} ", sem.value);
        if sem.parent.wait_list.is_empty() {
            println!("{}", sem.parent.wait_list.size());
        } else {
            print!("{}:", sem.parent.wait_list.size());
            let head = &sem.parent.wait_list;
            list_head_for_each!(node, head, {
                let thread = crate::thread_list_node_entry!(node.as_ptr()) as *mut RtThread;
                let _ = crate::format_name!((*thread).parent.name.as_ptr(), 8);
            });
            print!("\n");
        }
    };
    let _ = KObjectBase::get_info(
        callback_forword,
        callback,
        ObjectClassType::ObjectClassSemaphore as u8,
    );
}
