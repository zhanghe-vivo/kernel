use crate::{
    cpu::Cpu,
    impl_kobject, list_head_for_each,
    object::*,
    print, println,
    rt_bindings::{
        rt_debug_not_in_interrupt, rt_debug_scheduler_available, rt_err_t, rt_int32_t, rt_object,
        rt_object_hook_call, rt_size_t, rt_ssize_t, rt_ubase_t, rt_uint32_t, rt_uint8_t, RT_EFULL,
        RT_EINTR, RT_EINVAL, RT_ENOMEM, RT_EOK, RT_ERROR, RT_ETIMEOUT, RT_INTERRUPTIBLE,
        RT_IPC_CMD_RESET, RT_IPC_FLAG_FIFO, RT_IPC_FLAG_PRIO, RT_KILLABLE, RT_SEM_VALUE_MAX,
        RT_TIMER_CTRL_SET_TIME, RT_UNINTERRUPTIBLE, RT_WAITING_FOREVER,
    },
    sync::ipc_common::*,
    thread::RtThread,
    timer,
};
use blue_infra::list::doubly_linked_list::ListHead;
use core::pin::Pin;
use core::{ffi::c_void, marker::PhantomPinned, ptr::null_mut};

use crate::alloc::boxed::Box;
use core::cell::UnsafeCell;
use kernel::{fmt, str::CString};

use pinned_init::*;

#[pin_data(PinnedDrop)]
pub struct KSemaphore {
    #[pin]
    raw: UnsafeCell<RtSemaphore>,
    #[pin]
    pin: PhantomPinned,
}

unsafe impl Send for KSemaphore {}
unsafe impl Sync for KSemaphore {}

#[pinned_drop]
impl PinnedDrop for KSemaphore {
    fn drop(self: Pin<&mut Self>) {
        unsafe {
            (*self.raw.get()).detach();
        }
    }
}

impl KSemaphore {
    pub fn new(value: u32) -> Pin<Box<Self>> {
        fn init_raw(value: u16) -> impl PinInit<UnsafeCell<RtSemaphore>> {
            let init = move |slot: *mut UnsafeCell<RtSemaphore>| {
                let slot: *mut RtSemaphore = slot.cast();
                unsafe {
                    let cur_ref = &mut *slot;

                    if let Ok(s) = CString::try_from_fmt(fmt!("{:p}", slot)) {
                        cur_ref.init(s.as_ptr() as *const i8, value, RT_IPC_FLAG_PRIO as u8);
                    } else {
                        let default = "default";
                        cur_ref.init(default.as_ptr() as *const i8, value, RT_IPC_FLAG_PRIO as u8);
                    }
                }
                Ok(())
            };
            unsafe { pin_init_from_closure(init) }
        }

        Box::pin_init(pin_init!(Self {
            raw<-init_raw(value as u16),
            pin: PhantomPinned,
        }))
        .unwrap()
    }

    pub fn acquire(&self) -> KSemaphoreGuard<'_> {
        unsafe {
            (*self.raw.get()).take(RT_WAITING_FOREVER);
        };
        KSemaphoreGuard { sem: self }
    }
}
pub struct KSemaphoreGuard<'a> {
    sem: &'a KSemaphore,
}

impl<'a> Drop for KSemaphoreGuard<'a> {
    fn drop(&mut self) {
        unsafe { (*self.sem.raw.get()).release() };
    }
}

/// Semaphore raw structure
#[repr(C)]
#[pin_data]
pub struct RtSemaphore {
    /// Inherit from IPCObject
    #[pin]
    pub(crate) parent: IPCObject,
    /// Value of semaphore
    pub(crate) value: u16,
}

impl_kobject!(RtSemaphore);

impl RtSemaphore {
    #[inline]
    pub fn new(name: [i8; NAME_MAX], value: u16, flag: u8) -> impl PinInit<Self> {
        assert!((flag == RT_IPC_FLAG_FIFO as u8) || (flag == RT_IPC_FLAG_PRIO as u8));

        rt_debug_not_in_interrupt!();

        pin_init!(Self {
            parent<-IPCObject::new(ObjectClassType::ObjectClassSemaphore as u8, name, flag),
            value: value,
        })
    }

    #[inline]
    pub fn init(&mut self, name: *const i8, value: u16, flag: u8) {
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
    pub fn new_raw(name: *const i8, value: u16, flag: u8) -> *mut Self {
        let semaphore = Box::pin_init(RtSemaphore::new(char_ptr_to_array(name), value, flag));
        match semaphore {
            Ok(sem) => unsafe { Box::leak(Pin::into_inner_unchecked(sem)) },
            Err(_) => return null_mut(),
        }
    }

    #[inline]
    pub fn delete_raw(&mut self) {
        assert_eq!(
            self.type_name(),
            ObjectClassType::ObjectClassSemaphore as u8
        );
        assert!(!self.is_static_kobject());

        rt_debug_not_in_interrupt!();

        self.parent.wake_all();
        self.parent.parent.delete();
    }

    fn take_internal(&mut self, timeout: i32, suspend_flag: u32) -> i32 {
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

                (*thread).error = -(RT_EINTR as i32);

                let ret = self
                    .parent
                    .wait(thread, self.parent.flag, suspend_flag as u32);

                if ret != RT_EOK as i32 {
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

                if (*thread).error != RT_EOK as i32 {
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

        RT_EOK as i32
    }

    pub fn take(&mut self, timeout: i32) -> i32 {
        self.take_internal(timeout, RT_UNINTERRUPTIBLE as u32)
    }

    pub fn try_take(&mut self) -> i32 {
        self.take(0)
    }

    pub fn take_interruptible(&mut self, timeout: i32) -> i32 {
        self.take_internal(timeout, RT_INTERRUPTIBLE as u32)
    }

    pub fn take_killable(&mut self, timeout: i32) -> i32 {
        self.take_internal(timeout, RT_KILLABLE as u32)
    }

    pub fn release(&mut self) -> i32 {
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
                return -(RT_EFULL as i32);
            }
        }

        self.parent.unlock();

        if need_schedule {
            Cpu::get_current_scheduler().do_task_schedule();
        }

        RT_EOK as i32
    }

    pub fn control(&mut self, cmd: i32, arg: *const c_void) -> i32 {
        assert_eq!(
            self.type_name(),
            ObjectClassType::ObjectClassSemaphore as u8
        );

        if cmd == RT_IPC_CMD_RESET as i32 {
            self.parent.lock();

            self.parent.wake_all();

            self.value = arg as u16;

            self.parent.unlock();

            Cpu::get_current_scheduler().do_task_schedule();

            return RT_EOK as i32;
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
    RtSemaphore::new_raw(name, value as u16, flag)
}

#[cfg(all(feature = "RT_USING_SEMAPHORE", feature = "RT_USING_HEAP"))]
#[no_mangle]
pub unsafe extern "C" fn rt_sem_delete(sem: *mut RtSemaphore) -> rt_err_t {
    assert!(!sem.is_null());
    (*sem).delete_raw();
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
