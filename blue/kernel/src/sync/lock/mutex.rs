use crate::alloc::boxed::Box;
use crate::cpu::Cpu;
use crate::error::code;
use crate::impl_kobject;
use crate::object::*;
use crate::rt_bindings::{
    rt_debug_in_thread_context, rt_debug_not_in_interrupt, rt_debug_scheduler_available, rt_err_t,
    rt_int32_t, rt_object, rt_object_hook_call, rt_set_errno, RT_EFULL, RT_EINVAL, RT_EOK,
    RT_ERROR, RT_ETIMEOUT, RT_INTERRUPTIBLE, RT_IPC_FLAG_FIFO, RT_IPC_FLAG_PRIO, RT_KILLABLE,
    RT_MUTEX_HOLD_MAX, RT_THREAD_PRIORITY_MAX,
    RT_TIMER_CTRL_SET_TIME, RT_UNINTERRUPTIBLE, RT_WAITING_FOREVER, RT_WAITING_NO,
};
use crate::sync::ipc_common::*;
use crate::thread::RtThread;
use crate::{current_thread_ptr, list_head_for_each, print, println};
use blue_infra::list::doubly_linked_list::ListHead;

use core::ffi;
use core::marker::PhantomPinned;
use core::pin::Pin;
use core::ptr::null_mut;
use core::ptr::NonNull;
use core::{cell::UnsafeCell, ops::Deref, ops::DerefMut};
use kernel::{fmt, str::CString};
use pinned_init::*;

#[pin_data(PinnedDrop)]
pub struct KMutex<T> {
    #[pin]
    raw: UnsafeCell<RtMutex>,
    data_: UnsafeCell<T>,
    #[pin]
    pin: PhantomPinned,
}

unsafe impl<T: Send> Send for KMutex<T> {}
unsafe impl<T: Send> Sync for KMutex<T> {}

#[pinned_drop]
impl<T> PinnedDrop for KMutex<T> {
    fn drop(self: Pin<&mut Self>) {
        unsafe {
            (*self.raw.get()).detach();
        }
    }
}

impl<T> KMutex<T> {
    pub fn new(data: T) -> Pin<Box<Self>> {
        fn init_raw() -> impl PinInit<UnsafeCell<RtMutex>> {
            let init = |slot: *mut UnsafeCell<RtMutex>| {
                let slot: *mut RtMutex = slot.cast();
                unsafe {
                    let cur_ref = &mut *slot;

                    if let Ok(s) = CString::try_from_fmt(fmt!("{:p}", slot)) {
                        cur_ref.init(s.as_ptr() as *const i8, 0);
                    } else {
                        let default = "default";
                        cur_ref.init(default.as_ptr() as *const i8, 0);
                    }
                }
                Ok(())
            };
            unsafe { pin_init_from_closure(init) }
        }

        Box::pin_init(pin_init!(Self {
            data_: UnsafeCell::new(data),
            raw<-init_raw(),
            pin: PhantomPinned,
        }))
        .unwrap()
    }

    pub fn lock(&self) -> KMutexGuard<'_, T> {
        unsafe {
            (*self.raw.get()).take(RT_WAITING_FOREVER);
        };
        KMutexGuard { mtx: self }
    }
}

pub struct KMutexGuard<'a, T> {
    mtx: &'a KMutex<T>,
}

impl<'a, T> Drop for KMutexGuard<'a, T> {
    fn drop(&mut self) {
        unsafe { (*self.mtx.raw.get()).release() };
    }
}

impl<'a, T> Deref for KMutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.mtx.data_.get() }
    }
}

impl<'a, T> DerefMut for KMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.mtx.data_.get() }
    }
}

#[repr(C)]
#[pin_data]
pub struct RtMutex {
    /// Inherit from IPCObject
    #[pin]
    pub(crate) parent: IPCObject,
    /// Priority ceiling of mutex
    pub(crate) ceiling_priority: u8,
    /// Maximal priority for pending thread
    pub(crate) priority: u8,
    /// Numbers of thread hold the mutex
    pub(crate) hold: u8,
    /// Current owner of mutex
    pub(crate) owner: *mut RtThread,
    /// The object list taken by thread
    #[pin]
    pub(crate) taken_list: ListHead,
}

impl_kobject!(RtMutex);

impl RtMutex {
    #[inline]
    pub fn new(name: [i8; NAME_MAX], flag: u8) -> impl PinInit<Self> {
        assert!((flag == RT_IPC_FLAG_FIFO as u8) || (flag == RT_IPC_FLAG_PRIO as u8));

        rt_debug_not_in_interrupt!();

        pin_init!(Self {
            parent<-IPCObject::new(ObjectClassType::ObjectClassMutex as u8, name, flag),
            owner : null_mut(),
            priority : 0xFF,
            hold : 0,
            ceiling_priority : 0xFF,
            taken_list<-ListHead::new()
        })
    }
    #[inline]
    pub fn init(&mut self, name: *const i8, _flag: u8) {
        // Flag can only be RT_IPC_FLAG_PRIO.
        self.parent.init(
            ObjectClassType::ObjectClassMutex as u8,
            name,
            RT_IPC_FLAG_PRIO as u8,
        );

        self.owner = null_mut();
        self.priority = 0xFF;
        self.hold = 0;
        self.ceiling_priority = 0xFF;

        unsafe {
            let _ = ListHead::new().__pinned_init(&mut self.taken_list);
        }
    }

    #[inline]
    pub fn detach(&mut self) {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassMutex as u8);
        assert!(self.is_static_kobject());

        self.parent.lock();

        self.parent.wake_all();

        unsafe {
            Pin::new_unchecked(&mut self.taken_list).remove();
        }

        self.parent.unlock();

        Cpu::get_current_scheduler().do_task_schedule();

        self.parent.parent.detach();
    }

    #[inline]
    pub fn new_raw(name: *const i8, flag: u8) -> *mut Self {
        let mutex = Box::pin_init(RtMutex::new(char_ptr_to_array(name), flag));
        match mutex {
            Ok(mtx) => unsafe { Box::leak(Pin::into_inner_unchecked(mtx)) },
            Err(_) => return null_mut(),
        }
    }

    #[inline]
    pub fn delete_raw(&mut self) {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassMutex as u8);
        assert!(!self.is_static_kobject());

        rt_debug_not_in_interrupt!();

        self.parent.lock();
        self.parent.wake_all();

        unsafe {
            Pin::new_unchecked(&mut self.taken_list).remove();
        }
        self.parent.unlock();

        self.parent.parent.delete();
    }

    fn take_internal(&mut self, timeout: i32, suspend_flag: u32) -> i32 {
        // Shadow timeout for mutability
        let mut timeout = timeout;
        rt_debug_scheduler_available!(true);
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassMutex as u8);

        let thread_ptr = current_thread_ptr!();
        assert!(!thread_ptr.is_null());
        let thread = unsafe { &mut *thread_ptr };
        self.parent.lock();

        unsafe {
            rt_object_hook_call!(
                rt_object_trytake_hook,
                &mut self.parent.parent as *mut KObjectBase as *mut rt_object
            );
        }

        thread.error = code::EOK;

        if self.owner == thread_ptr {
            if self.hold < RT_MUTEX_HOLD_MAX as u8 {
                // Same thread
                self.hold += 1;
            } else {
                self.parent.unlock();
                return -(RT_EFULL as i32);
            }
        } else {
            // Whether the mutex has owner thread.
            if self.owner.is_null() {
                // Set mutex owner and original priority
                self.owner = thread_ptr;
                self.priority = 0xff;
                self.hold = 1;
                let mutex_owner = unsafe { &mut *self.owner };

                if self.ceiling_priority != 0xFF {
                    // Set the priority of thread to the ceiling priority
                    if self.ceiling_priority < mutex_owner.priority.get_current() {
                        mutex_owner.update_priority(self.ceiling_priority, suspend_flag as u32);
                    }
                }

                // SAFETY: list should be ensured safe
                // Insert mutex to thread's taken object list
                unsafe {
                    Pin::new_unchecked(&mut thread.mutex_info.taken_list)
                        .insert_next(&mut self.taken_list);
                }
            } else {
                // No waiting, return with timeout
                if timeout == 0 {
                    thread.error = code::ETIMEOUT;

                    self.parent.unlock();

                    return -(RT_ETIMEOUT as i32);
                } else {
                    let mut priority = thread.priority.get_current();
                    // Suspend current thread
                    let mut ret =
                        self.parent
                            .wait(thread_ptr, self.parent.flag, suspend_flag as u32);
                    if ret != RT_EOK as i32 {
                        self.parent.unlock();
                        return ret;
                    }

                    // Set pending object in thread to this mutex
                    thread.mutex_info.pending_to = unsafe { Some(NonNull::new_unchecked(self)) };

                    // Update the priority level of mutex
                    if priority < self.priority {
                        self.priority = priority;
                        let mutex_owner = unsafe { &mut *self.owner };
                        if self.priority < mutex_owner.priority.get_current() {
                            mutex_owner.update_priority(priority, RT_UNINTERRUPTIBLE);
                        }
                    }

                    if timeout > 0 {
                        thread.thread_timer.timer_control(
                            RT_TIMER_CTRL_SET_TIME,
                            (&mut timeout) as *mut i32 as *mut ffi::c_void,
                        );

                        thread.thread_timer.start();
                    }

                    self.parent.unlock();

                    Cpu::get_current_scheduler().do_task_schedule();

                    self.parent.lock();

                    if thread.error != code::EOK {
                        // The mutex has not been taken and thread has detached
                        // from the pending list.
                        let mut need_update = false;

                        unsafe {
                            if !self.owner.is_null()
                                && (*self.owner).priority.get_current()
                                    == thread.priority.get_current()
                            {
                                need_update = true;
                            }

                            if let Some(node) = self.parent.wait_list.next() {
                                let th =
                                    crate::thread_list_node_entry!(node.as_ptr()) as *mut RtThread;
                                self.priority = (*th).priority.get_current();
                            } else {
                                self.priority = 0xff;
                            }
                        }

                        // Try to change the priority of mutex owner if necessary
                        if need_update {
                            // SAFETY: self owner is not null
                            let mutex_owner = unsafe { &mut *self.owner };
                            priority = mutex_owner.get_mutex_priority();
                            if priority != mutex_owner.priority.get_current() {
                                mutex_owner.update_priority(priority, RT_UNINTERRUPTIBLE);
                            }
                        }

                        self.parent.unlock();

                        thread.mutex_info.pending_to = None;

                        ret = thread.error.to_errno();
                        return if ret > 0 { -ret } else { ret };
                    }
                }
            }
        }

        self.parent.unlock();

        unsafe {
            rt_object_hook_call!(
                rt_object_take_hook,
                &self.parent.parent as *const KObjectBase as *const rt_object
            );
        }

        RT_EOK as i32
    }

    pub fn take(&mut self, time: i32) -> i32 {
        self.take_internal(time, RT_UNINTERRUPTIBLE as u32)
    }

    pub fn take_interruptible(&mut self, time: i32) -> i32 {
        self.take_internal(time, RT_INTERRUPTIBLE as u32)
    }

    pub fn take_killable(&mut self, time: i32) -> i32 {
        self.take_internal(time, RT_KILLABLE as u32)
    }

    pub fn try_take(&mut self) -> i32 {
        self.take(RT_WAITING_NO as i32)
    }

    pub fn release(&mut self) -> i32 {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassMutex as u8);

        //Only thread could release mutex because we need test the ownership
        rt_debug_in_thread_context!();

        let thread_ptr = current_thread_ptr!();
        if thread_ptr.is_null() {
            return -(RT_ERROR as i32);
        }
        let thread = unsafe { &mut *thread_ptr };

        self.parent.lock();

        unsafe {
            rt_object_hook_call!(
                rt_object_put_hook,
                &mut self.parent.parent as *mut KObjectBase as *mut rt_object
            );
        }

        if thread_ptr != self.owner {
            thread.error = code::ERROR;
            self.parent.unlock();
            return -(RT_ERROR as i32);
        }

        self.hold -= 1;
        let mut need_schedule = false;

        if self.hold == 0 {
            unsafe {
                Pin::new_unchecked(&mut self.taken_list).remove();
            }

            if self.ceiling_priority != 0xFF || thread.priority.get_current() == self.priority {
                let priority = thread.get_mutex_priority();

                thread.change_priority(priority);

                need_schedule = true;
            }

            if !self.parent.wait_list.is_empty() {
                let mut next_thread_ptr = null_mut();

                if let Some(node) = self.parent.wait_list.next() {
                    next_thread_ptr = unsafe { crate::thread_list_node_entry!(node.as_ptr()) };
                    if next_thread_ptr.is_null() {
                        return -(RT_ERROR as i32);
                    }
                } else {
                    return -(RT_ERROR as i32);
                }

                let next_thread = unsafe { &mut *next_thread_ptr };

                unsafe { Pin::new_unchecked(&mut next_thread.tlist).remove() };

                self.owner = next_thread_ptr;
                self.hold = 1;

                unsafe {
                    Pin::new_unchecked(&mut next_thread.mutex_info.taken_list)
                        .insert_next(&self.taken_list)
                };

                next_thread.mutex_info.pending_to = None;
                next_thread.resume();

                if self.parent.has_waiting() {
                    let mut th = null_mut();
                    if let Some(node) = self.parent.wait_list.next() {
                        th = unsafe { crate::thread_list_node_entry!(node.as_ptr()) };
                        if th.is_null() {
                            return -(RT_ERROR as i32);
                        }
                    } else {
                        return -(RT_ERROR as i32);
                    }

                    unsafe {
                        self.priority = (*th).priority.get_current();
                    }
                } else {
                    self.priority = 0xff;
                }

                need_schedule = true;
            } else {
                self.owner = null_mut();
                self.priority = 0xff;
            }
        }

        self.parent.unlock();

        if need_schedule {
            Cpu::get_current_scheduler().do_task_schedule();
        }

        RT_EOK as i32
    }

    #[inline]
    pub(crate) fn update_priority(&mut self) -> u8 {
        if let Some(node) = self.parent.wait_list.next() {
            unsafe {
                let thread: *mut RtThread = crate::thread_list_node_entry!(node.as_ptr());
                self.priority = (*thread).priority.get_current();
            }
        } else {
            self.priority = 0xff;
        }

        self.priority
    }

    pub(crate) fn drop_thread(&mut self, thread_ptr: *mut RtThread) {
        if thread_ptr.is_null() {
            return;
        }
        // SAFETY: thread is null checked
        let thread = unsafe { &mut *thread_ptr };

        let mut priority: u8 = 0;
        let mut need_update = false;

        thread.remove_tlist();

        if self.owner.is_null() {
            return;
        }

        // SAFETY: owner is null checked
        let mutex_owner = unsafe { &mut *self.owner };

        if mutex_owner.priority.get_current() == thread.priority.get_current() {
            need_update = true;
        }

        if let Some(node) = self.parent.wait_list.next() {
            unsafe {
                let th: *mut RtThread = crate::thread_list_node_entry!(node.as_ptr());
                self.priority = (*th).priority.get_current();
            }
        } else {
            self.priority = 0xff;
        }

        if need_update {
            priority = mutex_owner.get_mutex_priority();
            if priority != mutex_owner.priority.get_current() {
                mutex_owner.update_priority(priority, RT_UNINTERRUPTIBLE as u32);
            }
        }
    }

    pub(crate) fn set_prio_ceiling(&mut self, priority: u8) -> u8 {
        let mut prev_priority: u8 = 0xFF;

        if priority < RT_THREAD_PRIORITY_MAX as u8 {
            //Critical section here if multiple updates to one mutex happen concurrently
            self.parent.lock();
            prev_priority = self.ceiling_priority;
            self.ceiling_priority = priority;
            let owner_thread = self.owner;
            if !owner_thread.is_null() {
                // SAFETY: owner_thread is null checked
                unsafe {
                    let priority = (*owner_thread).get_mutex_priority();
                    if priority != (*owner_thread).priority.get_current() {
                        (*owner_thread).update_priority(priority, RT_UNINTERRUPTIBLE as u32);
                    }
                }
            }
            self.parent.unlock();
        } else {
            unsafe {
                rt_set_errno(-(RT_EINVAL as rt_err_t));
            }
        }

        prev_priority
    }

    pub(crate) fn get_prio_ceiling(&self) -> u8 {
        self.ceiling_priority
    }
}

#[cfg(feature = "RT_USING_MUTEX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mutex_init(
    mutex: *mut RtMutex,
    name: *const ffi::c_char,
    _flag: ffi::c_uchar,
) -> rt_err_t {
    assert!(!mutex.is_null());

    (*mutex).init(name, _flag);

    RT_EOK as rt_err_t
}

#[cfg(feature = "RT_USING_MUTEX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mutex_detach(mutex: *mut RtMutex) -> rt_err_t {
    assert!(!mutex.is_null());

    (*mutex).detach();

    RT_EOK as rt_err_t
}

#[cfg(all(feature = "RT_USING_MUTEX", feature = "RT_USING_HEAP"))]
#[no_mangle]
pub unsafe extern "C" fn rt_mutex_create(
    name: *const ffi::c_char,
    _flag: ffi::c_uchar,
) -> *mut RtMutex {
    RtMutex::new_raw(name, _flag)
}

#[cfg(all(feature = "RT_USING_MUTEX", feature = "RT_USING_HEAP"))]
#[no_mangle]
pub unsafe extern "C" fn rt_mutex_delete(mutex: *mut RtMutex) -> rt_err_t {
    assert!(!mutex.is_null());
    (*mutex).delete_raw();
    RT_EOK as rt_err_t
}

#[cfg(feature = "RT_USING_MUTEX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mutex_take(mutex: *mut RtMutex, time: rt_int32_t) -> rt_err_t {
    assert!(!mutex.is_null());
    (*mutex).take(time)
}

#[cfg(feature = "RT_USING_MUTEX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mutex_take_interruptible(
    mutex: *mut RtMutex,
    time: rt_int32_t,
) -> rt_err_t {
    assert!(!mutex.is_null());
    (*mutex).take_interruptible(time)
}

#[cfg(feature = "RT_USING_MUTEX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mutex_take_killable(mutex: *mut RtMutex, time: rt_int32_t) -> rt_err_t {
    assert!(!mutex.is_null());
    (*mutex).take_killable(time)
}

#[cfg(feature = "RT_USING_MUTEX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mutex_trytake(mutex: *mut RtMutex) -> rt_err_t {
    assert!(!mutex.is_null());
    (*mutex).try_take()
}

#[cfg(feature = "RT_USING_MUTEX")]
#[no_mangle]
pub unsafe extern "C" fn rt_mutex_release(mutex: *mut RtMutex) -> rt_err_t {
    assert!(!mutex.is_null());
    (*mutex).release()
}
#[no_mangle]
#[allow(unused_unsafe)]
pub extern "C" fn rt_mutex_info() {
    let callback_forword = || {
        println!("mutex      owner  hold priority suspend thread");
        println!("-------- -------- ---- -------- --------------");
    };
    let callback = |node: &ListHead| unsafe {
        let mutex = &*(crate::list_head_entry!(node.as_ptr(), KObjectBase, list) as *const RtMutex);
        let _ = crate::format_name!(mutex.parent.parent.name.as_ptr(), 8);
        if mutex.owner.is_null() {
            print!(" (NULL)   ");
        } else {
            let _ = crate::format_name!((*mutex.owner).parent.name.as_ptr(), 8);
        }
        print!("{:04}", mutex.hold);
        print!("{:>8}  ", mutex.priority);
        if mutex.parent.wait_list.is_empty() {
            println!("0000");
        } else {
            print!("{}:", mutex.parent.wait_list.size());
            let head = &mutex.parent.wait_list;
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
        ObjectClassType::ObjectClassMutex as u8,
    );
}
