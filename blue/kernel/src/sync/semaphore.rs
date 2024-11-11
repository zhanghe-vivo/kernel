use crate::{
    cpu::Cpu,
    error::Error,
    linked_list::ListHead,
    list_head_for_each,
    object::{
        rt_object_allocate, rt_object_delete, rt_object_detach, rt_object_get_type,
        rt_object_is_systemobject, ObjectClassType, *,
    },
    print, println,
    rt_bindings::*,
    rt_debug_not_in_interrupt,
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

use kernel::{
    //rt_bindings::{rt_hw_interrupt_disable, rt_hw_interrupt_enable},
    rt_debug_scheduler_available,
    rt_object_hook_call,
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
    /// Spin lock for protecting the semaphore value
    spinlock: RawSpin,
}

impl RtSemaphore {
    #[inline]
    fn new_internal(
        name: &'static CStr,
        value: ffi::c_ushort,
        flag: IpcFlagType,
        is_static: bool,
    ) -> impl PinInit<Self> {
        assert!(value < ffi::c_ushort::MAX);
        assert!(
            (flag == RT_IPC_FLAG_FIFO as IpcFlagType) || (flag == RT_IPC_FLAG_PRIO as IpcFlagType)
        );
        let init = move |slot: *mut Self| unsafe {
            let cur_ref = &mut *slot;
            let _ = IPCObject::new(name, ObjectClassType::ObjectClassSemaphore, flag, is_static)
                .__pinned_init(&mut cur_ref.parent);

            if is_static == false {
                cur_ref.spinlock = RawSpin::new();
            }

            cur_ref.value = value;
            Ok(())
        };
        unsafe { pin_init_from_closure(init) }
    }

    #[inline]
    fn static_new(
        name: &'static CStr,
        value: ffi::c_ushort,
        flag: IpcFlagType,
    ) -> impl PinInit<Self> {
        Self::new_internal(name, value, flag, true)
    }

    #[inline]
    fn dyn_new(name: &'static CStr, value: ffi::c_ushort, flag: IpcFlagType) -> impl PinInit<Self> {
        Self::new_internal(name, value, flag, false)
    }

    #[inline]
    pub fn init(
        &mut self,
        name: *const core::ffi::c_char,
        value: rt_uint32_t,
        flag: rt_uint8_t,
    ) -> rt_err_t {
        unsafe {
            let _ = RtSemaphore::static_new(
                CStr::from_char_ptr(name),
                value as ffi::c_ushort,
                flag as IpcFlagType,
            )
            .__pinned_init(self as *mut Self);
        }
        RT_EOK as rt_err_t
    }

    #[inline]
    pub fn detach(&mut self) -> rt_err_t {
        _ipc_list_resume_all(&mut (self.parent.suspend_thread));
        rt_object_detach(&mut self.parent.parent as *mut KObjectBase as *mut rt_object);

        RT_EOK as rt_err_t
    }

    #[inline]
    pub fn new(name: *const core::ffi::c_char, value: rt_uint32_t, flag: rt_uint8_t) -> *mut Self {
        unsafe {
            let sem = rt_object_allocate(ObjectClassType::ObjectClassSemaphore as u32, name)
                as *mut RtSemaphore;

            let _ = RtSemaphore::dyn_new(
                CStr::from_char_ptr(name),
                value as ffi::c_ushort,
                flag as IpcFlagType,
            )
            .__pinned_init(sem);

            sem
        }
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
    assert!(sem != null_mut());
    assert!(value < 0x10000 as rt_uint32_t);
    assert!((flag == RT_IPC_FLAG_FIFO as rt_uint8_t) || (flag == RT_IPC_FLAG_PRIO as rt_uint8_t));

    (*sem).init(name, value, flag);

    RT_EOK as rt_err_t
}
#[cfg(feature = "RT_USING_SEMAPHORE")]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_detach(sem: *mut RtSemaphore) -> rt_err_t {
    assert!(sem != null_mut());
    assert!(
        rt_object_get_type(&mut (*sem).parent.parent as *mut KObjectBase as *mut rt_object)
            == ObjectClassType::ObjectClassSemaphore as u8
    );
    assert!(
        rt_object_is_systemobject(&mut (*sem).parent.parent as *mut KObjectBase as *mut rt_object)
            == RT_TRUE as i32
    );

    (*sem).detach()
}
#[cfg(all(feature = "RT_USING_SEMAPHORE", feature = "RT_USING_HEAP"))]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_create(
    name: *const core::ffi::c_char,
    value: rt_uint32_t,
    flag: rt_uint8_t,
) -> *mut RtSemaphore {
    assert!(value < 0x10000 as rt_uint32_t);
    assert!((flag == RT_IPC_FLAG_FIFO as rt_uint8_t) || (flag == RT_IPC_FLAG_PRIO as rt_uint8_t));
    rt_debug_not_in_interrupt!();

    //RtSemaphore::new(name, value, flag)
    rt_debug_not_in_interrupt!();
    let sem = rt_object_allocate(ObjectClassType::ObjectClassSemaphore as u32, name) as rt_sem_t
        as *mut RtSemaphore;

    if sem == null_mut() {
        return sem;
    }

    _ipc_object_init(&mut (*sem).parent);

    (*sem).value = value as u16;
    (*sem).parent.flag = flag;

    sem
}

#[cfg(all(feature = "RT_USING_SEMAPHORE", feature = "RT_USING_HEAP"))]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_delete(sem: *mut RtSemaphore) -> rt_err_t {
    assert!(sem != null_mut());
    assert!(
        rt_object_get_type(&mut (*sem).parent.parent as *mut KObjectBase as *mut rt_object)
            == ObjectClassType::ObjectClassSemaphore as u8
    );
    assert!(
        rt_object_is_systemobject(&mut (*sem).parent.parent as *mut KObjectBase as *mut rt_object)
            == RT_FALSE as i32
    );

    rt_debug_not_in_interrupt!();

    _ipc_list_resume_all(&mut ((*sem).parent.suspend_thread));

    rt_object_delete(&mut (*sem).parent.parent as *mut KObjectBase as *mut rt_object);

    RT_EOK as rt_err_t
}

#[cfg(feature = "RT_USING_SEMAPHORE")]
#[no_mangle]
unsafe extern "C" fn _rt_sem_take(
    sem: *mut RtSemaphore,
    timeout: rt_int32_t,
    suspend_flag: i32,
) -> rt_err_t {
    let mut time_out = timeout as i32;
    assert!(sem != null_mut());
    assert!(
        rt_object_get_type((&mut (*sem).parent.parent) as *mut KObjectBase as *mut rt_object)
            == ObjectClassType::ObjectClassSemaphore as u8
    );

    rt_object_hook_call!(
        rt_object_trytake_hook,
        &mut (*sem).parent.parent as *mut KObjectBase as *mut rt_object
    );

    #[allow(unused_variables)]
    let check = (*sem).value == 0 && timeout != 0;
    rt_debug_scheduler_available!(check);

    let level = rt_hw_interrupt_disable();

    if (*sem).value > 0 {
        (*sem).value -= 1;
        rt_hw_interrupt_enable(level);
    } else {
        if timeout == 0 {
            rt_hw_interrupt_enable(level);

            /* FIXME: -2 is as expected, while C -RT_ETIMEOUT is -116. */
            return -116; //(RT_ETIMEOUT as rt_err_t);
        } else {
            let thread = unsafe { crate::current_thread!().unwrap().as_mut() };

            (*thread).error = -(RT_EINTR as rt_err_t);

            let ret = _ipc_list_suspend(
                &mut ((*sem).parent.suspend_thread),
                thread,
                (*sem).parent.flag,
                suspend_flag as u32,
            );

            if ret != RT_EOK as rt_err_t {
                rt_hw_interrupt_enable(level);
                return ret;
            }

            if timeout > 0 {
                timer::rt_timer_control(
                    &mut (*thread).thread_timer as *const _ as *mut timer::Timer,
                    RT_TIMER_CTRL_SET_TIME as i32,
                    (&mut time_out) as *mut i32 as *mut c_void,
                );
                timer::rt_timer_start(
                    &mut ((*thread).thread_timer) as *const _ as *mut timer::Timer,
                );
            }

            rt_hw_interrupt_enable(level);

            Cpu::get_current_scheduler().do_task_schedule();

            if (*thread).error != RT_EOK as rt_err_t {
                return (*thread).error;
            }
        }
    }

    rt_object_hook_call!(
        rt_object_take_hook,
        &mut (*sem).parent.parent as *mut KObjectBase as *mut rt_object
    );

    RT_EOK as rt_err_t
}

#[cfg(feature = "RT_USING_SEMAPHORE")]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_take(sem: *mut RtSemaphore, time: rt_int32_t) -> rt_err_t {
    _rt_sem_take(sem, time, RT_UNINTERRUPTIBLE as i32)
}

#[cfg(feature = "RT_USING_SEMAPHORE")]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_take_interruptible(
    sem: *mut RtSemaphore,
    time: rt_int32_t,
) -> rt_err_t {
    _rt_sem_take(sem, time, RT_INTERRUPTIBLE as i32)
}

#[cfg(feature = "RT_USING_SEMAPHORE")]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_take_killable(sem: *mut RtSemaphore, time: rt_int32_t) -> rt_err_t {
    _rt_sem_take(sem, time, RT_KILLABLE as i32)
}

#[cfg(feature = "RT_USING_SEMAPHORE")]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_trytake(sem: *mut RtSemaphore) -> rt_err_t {
    rt_sem_take(sem, RT_WAITING_NO as i32)
}

#[cfg(feature = "RT_USING_SEMAPHORE")]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_release(sem: *mut RtSemaphore) -> rt_err_t {
    assert!(sem != null_mut());
    assert!(
        rt_object_get_type(&mut (*sem).parent.parent as *mut KObjectBase as *mut rt_object)
            == ObjectClassType::ObjectClassSemaphore as u8
    );
    rt_object_hook_call!(
        rt_object_put_hook,
        &mut (*sem).parent.parent as *mut KObjectBase as *mut rt_object
    );

    let mut need_schedule = RT_FALSE;
    let level = rt_hw_interrupt_disable();

    if (*sem).parent.suspend_thread.is_empty() == false {
        _ipc_list_resume(&mut ((*sem).parent.suspend_thread));
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
        Cpu::get_current_scheduler().do_task_schedule();
    }

    RT_EOK as rt_err_t
}

#[cfg(feature = "RT_USING_SEMAPHORE")]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_control(
    sem: *mut RtSemaphore,
    cmd: i32,
    arg: *const c_void,
) -> rt_err_t {
    assert!(sem != null_mut());
    assert!(
        rt_object_get_type(&mut (*sem).parent.parent as *mut KObjectBase as *mut rt_object)
            == ObjectClassType::ObjectClassSemaphore as u8
    );

    if cmd == RT_IPC_CMD_RESET as i32 {
        let value = arg as rt_ubase_t;
        let level = rt_hw_interrupt_disable();

        _ipc_list_resume_all(&mut (*sem).parent.suspend_thread);

        (*sem).value = value as rt_uint16_t;

        rt_hw_interrupt_enable(level);

        Cpu::get_current_scheduler().do_task_schedule();

        return RT_EOK as rt_err_t;
    }

    -(RT_ERROR as rt_err_t)
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
        if sem.parent.suspend_thread.is_empty() {
            println!("{}", sem.parent.suspend_thread.size());
        } else {
            print!("{}:", sem.parent.suspend_thread.size());
            let head = &sem.parent.suspend_thread;
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
