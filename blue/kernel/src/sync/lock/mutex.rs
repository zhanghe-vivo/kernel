use crate::{
    alloc::boxed::Box,
    blue_kconfig::THREAD_PRIORITY_MAX,
    clock::WAITING_FOREVER,
    cpu::Cpu,
    current_thread_ptr,
    error::{code, set_errno},
    impl_kobject,
    object::*,
    sync::ipc_common::*,
    thread::{RtThread, SuspendFlag},
    timer::TimerControlAction,
};
use blue_infra::list::doubly_linked_list::ListHead;

use core::{
    cell::UnsafeCell,
    ffi,
    marker::PhantomPinned,
    mem,
    ops::{Deref, DerefMut},
    pin::Pin,
    ptr::{null_mut, NonNull},
};
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

pub const MUTEX_HOLD_MAX: u32 = 255;
pub const WAITING_NO: u32 = 0;

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
            (*self.raw.get()).lock();
        };
        KMutexGuard { mtx: self }
    }
}

pub struct KMutexGuard<'a, T> {
    mtx: &'a KMutex<T>,
}

impl<'a, T> Drop for KMutexGuard<'a, T> {
    fn drop(&mut self) {
        unsafe { (*self.mtx.raw.get()).unlock() };
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
    /// Inherit from KObjectBase
    #[pin]
    pub(crate) parent: KObjectBase,
    /// Priority ceiling of mutex
    pub(crate) ceiling_priority: u8,
    /// Maximal priority for pending thread
    pub(crate) priority: u8,
    /// Current owner of mutex
    pub(crate) owner: *mut RtThread,
    /// The object list taken by thread
    #[pin]
    pub(crate) taken_list: ListHead,
    /// SysQueue for mutex
    #[pin]
    pub(crate) inner_queue: RtSysQueue,
}

impl_kobject!(RtMutex);

impl RtMutex {
    #[inline]
    pub fn new(name: [i8; NAME_MAX], _waiting_mode: u8) -> impl PinInit<Self> {
        crate::debug_not_in_interrupt!();

        let init = move |slot: *mut Self| unsafe {
            let cur_ref = &mut *slot;
            let _ = KObjectBase::new(ObjectClassType::ObjectClassMutex as u8, name)
                .__pinned_init(&mut cur_ref.parent as *mut KObjectBase);

            cur_ref.owner = null_mut();
            cur_ref.priority = 0xFF;
            cur_ref.ceiling_priority = 0xFF;
            let _ = ListHead::new().__pinned_init(&mut cur_ref.taken_list as *mut ListHead);

            let _ = RtSysQueue::new(
                mem::size_of::<u32>(),
                1,
                IPC_SYS_QUEUE_STUB,
                IPC_WAIT_MODE_PRIO as u32,
            )
            .__pinned_init(&mut cur_ref.inner_queue as *mut RtSysQueue);
            Ok(())
        };
        unsafe { pin_init_from_closure(init) }
    }
    #[inline]
    pub fn init(&mut self, name: *const i8, _waiting_mode: u8) {
        // Flag can only be IPC_WAIT_MODE_PRIO.
        self.parent
            .init(ObjectClassType::ObjectClassMutex as u8, name);

        self.init_internal();
    }

    #[inline]
    pub fn init_dyn(&mut self, name: *const i8, _waiting_mode: u8) {
        self.parent
            .init_dyn(ObjectClassType::ObjectClassMutex as u8, name);
        self.init_internal();
    }

    #[inline]
    pub fn init_internal(&mut self) {
        self.owner = null_mut();
        self.priority = 0xFF;
        self.ceiling_priority = 0xFF;

        unsafe {
            let _ = ListHead::new().__pinned_init(&mut self.taken_list);
        }

        let _ = unsafe {
            RtSysQueue::new(
                mem::size_of::<u32>(),
                1,
                IPC_SYS_QUEUE_STUB,
                IPC_WAIT_MODE_PRIO as u32,
            )
            .__pinned_init(&mut self.inner_queue as *mut RtSysQueue)
        };
    }

    #[inline]
    pub fn detach(&mut self) {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassMutex as u8);

        self.inner_queue.lock();

        self.inner_queue.enqueue_waiter.inner_locked_wake_all();

        unsafe {
            Pin::new_unchecked(&mut self.taken_list).remove();
        }

        self.inner_queue.unlock();

        Cpu::get_current_scheduler().do_task_schedule();

        if self.is_static_kobject() {
            self.parent.detach();
        }
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

        crate::debug_not_in_interrupt!();

        self.inner_queue.lock();
        self.inner_queue.enqueue_waiter.inner_locked_wake_all();

        unsafe {
            Pin::new_unchecked(&mut self.taken_list).remove();
        }
        self.inner_queue.unlock();

        self.parent.delete();
    }

    pub fn lock_internal(&mut self, timeout: i32, pending_mode: u32) -> i32 {
        // Shadow timeout for mutability
        let mut timeout = timeout;
        crate::debug_scheduler_available!(true);
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassMutex as u8);

        let thread_ptr = current_thread_ptr!();
        assert!(!thread_ptr.is_null());
        let thread = unsafe { &mut *thread_ptr };
        self.inner_queue.lock();

        thread.error = code::EOK;

        if self.owner == thread_ptr {
            if self.inner_queue.count() < IPC_MUTEX_NESTED_MAX as usize {
                // Same thread
                self.inner_queue.force_push_stub();
            } else {
                self.inner_queue.unlock();
                return code::EFULL.to_errno();
            }
        } else {
            // Whether the mutex has owner thread.
            if self.owner.is_null() {
                // Set mutex owner and original priority
                self.owner = thread_ptr;
                self.priority = 0xff;
                self.inner_queue.reset_stub(1);
                let mutex_owner = unsafe { &mut *self.owner };

                if self.ceiling_priority != 0xFF {
                    // Set the priority of thread to the ceiling priority

                    if self.ceiling_priority < mutex_owner.priority.get_current() {
                        mutex_owner.update_priority(self.ceiling_priority, pending_mode);
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

                    self.inner_queue.unlock();

                    return code::ETIMEOUT.to_errno();
                } else {
                    let mut priority = thread.priority.get_current();
                    // Suspend current thread
                    let mut ret = self.inner_queue.enqueue_waiter.wait(thread, pending_mode);
                    if ret != code::EOK.to_errno() {
                        self.inner_queue.unlock();
                        return ret;
                    }

                    // Set pending object in thread to this mutex
                    thread.mutex_info.pending_to = unsafe { Some(NonNull::new_unchecked(self)) };

                    // Update the priority level of mutex
                    if priority < self.priority {
                        self.priority = priority;
                        let mutex_owner = unsafe { &mut *self.owner };

                        if self.priority < mutex_owner.priority.get_current() {
                            mutex_owner
                                .update_priority(priority, SuspendFlag::Uninterruptible as u32);
                        }
                    }

                    if timeout > 0 {
                        thread.thread_timer.timer_control(
                            TimerControlAction::SetTime,
                            (&mut timeout) as *mut i32 as *mut ffi::c_void,
                        );

                        thread.thread_timer.start();
                    }

                    self.inner_queue.unlock();

                    Cpu::get_current_scheduler().do_task_schedule();

                    self.inner_queue.lock();

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

                            if let Some(node) = self.inner_queue.enqueue_waiter.head() {
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
                                mutex_owner
                                    .update_priority(priority, SuspendFlag::Uninterruptible as u32);
                            }
                        }

                        self.inner_queue.unlock();

                        thread.mutex_info.pending_to = None;

                        ret = thread.error.to_errno();

                        return if ret > 0 { -ret } else { ret };
                    }
                }
            }
        }

        self.inner_queue.unlock();

        code::EOK.to_errno()
    }

    pub fn lock(&mut self) -> i32 {
        self.lock_internal(WAITING_FOREVER as i32, SuspendFlag::Uninterruptible as u32)
    }

    pub fn lock_wait(&mut self, time: i32) -> i32 {
        self.lock_internal(time, SuspendFlag::Uninterruptible as u32)
    }

    pub fn try_lock(&mut self) -> i32 {
        self.lock_wait(WAITING_NO as i32)
    }

    pub fn unlock(&mut self) -> i32 {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassMutex as u8);

        //Only thread could release mutex because we need test the ownership
        crate::debug_in_thread_context!();

        let thread_ptr = current_thread_ptr!();
        if thread_ptr.is_null() {
            return code::ERROR.to_errno();
        }
        let thread = unsafe { &mut *thread_ptr };

        self.inner_queue.lock();

        if thread_ptr != self.owner {
            thread.error = code::ERROR;
            self.inner_queue.unlock();
            return code::ERROR.to_errno();
        }

        self.inner_queue.pop_stub();
        let mut need_schedule = false;

        if self.inner_queue.is_empty() {
            unsafe {
                Pin::new_unchecked(&mut self.taken_list).remove();
            }

            if self.ceiling_priority != 0xFF || thread.priority.get_current() == self.priority {
                let priority = thread.get_mutex_priority();

                thread.change_priority(priority);

                need_schedule = true;
            }

            if !self.inner_queue.enqueue_waiter.is_empty() {
                #[allow(unused_assignments)]
                let mut next_thread_ptr = null_mut();

                if let Some(node) = self.inner_queue.enqueue_waiter.head() {
                    next_thread_ptr = unsafe { crate::thread_list_node_entry!(node.as_ptr()) };
                    if next_thread_ptr.is_null() {
                        return code::ERROR.to_errno();
                    }
                } else {
                    return code::ERROR.to_errno();
                }

                let next_thread = unsafe { &mut *next_thread_ptr };

                unsafe { Pin::new_unchecked(&mut next_thread.tlist).remove() };

                self.owner = next_thread_ptr;
                self.inner_queue.reset_stub(1);

                unsafe {
                    Pin::new_unchecked(&mut next_thread.mutex_info.taken_list)
                        .insert_next(&self.taken_list)
                };

                next_thread.mutex_info.pending_to = None;
                next_thread.resume();

                if !self.inner_queue.enqueue_waiter.is_empty() {
                    #[allow(unused_assignments)]
                    let mut th = null_mut();
                    if let Some(node) = self.inner_queue.enqueue_waiter.head() {
                        th = unsafe { crate::thread_list_node_entry!(node.as_ptr()) };
                        if th.is_null() {
                            return code::ERROR.to_errno();
                        }
                    } else {
                        return code::ERROR.to_errno();
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

        self.inner_queue.unlock();

        if need_schedule {
            Cpu::get_current_scheduler().do_task_schedule();
        }

        code::EOK.to_errno()
    }

    #[inline]
    pub(crate) fn update_priority(&mut self) -> u8 {
        if let Some(node) = self.inner_queue.enqueue_waiter.head() {
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

        if let Some(node) = self.inner_queue.enqueue_waiter.head() {
            unsafe {
                let th: *mut RtThread = crate::thread_list_node_entry!(node.as_ptr());
                self.priority = (*th).priority.get_current();
            }
        } else {
            self.priority = 0xff;
        }

        if need_update {
            let priority = mutex_owner.get_mutex_priority();
            if priority != mutex_owner.priority.get_current() {
                mutex_owner.update_priority(priority, SuspendFlag::Uninterruptible as u32);
            }
        }
    }

    pub(crate) fn set_prio_ceiling(&mut self, priority: u8) -> u8 {
        let mut prev_priority: u8 = 0xFF;

        if priority < THREAD_PRIORITY_MAX as u8 {
            //Critical section here if multiple updates to one mutex happen concurrently
            self.inner_queue.lock();
            prev_priority = self.ceiling_priority;
            self.ceiling_priority = priority;
            let owner_thread = self.owner;
            if !owner_thread.is_null() {
                // SAFETY: owner_thread is null checked
                unsafe {
                    let priority = (*owner_thread).get_mutex_priority();

                    if priority != (*owner_thread).priority.get_current() {
                        (*owner_thread)
                            .update_priority(priority, SuspendFlag::Uninterruptible as u32);
                    }
                }
            }
            self.inner_queue.unlock();
        } else {
            unsafe {
                set_errno(code::EINVAL.to_errno());
            }
        }

        prev_priority
    }

    pub(crate) fn get_prio_ceiling(&self) -> u8 {
        self.ceiling_priority
    }
}
