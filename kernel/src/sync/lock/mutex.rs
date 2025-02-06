use crate::{
    alloc::boxed::Box,
    blue_kconfig::THREAD_PRIORITY_MAX,
    clock::WAITING_FOREVER,
    cpu::Cpu,
    current_thread_ptr,
    error::{code, set_errno, Error},
    impl_kobject,
    object::*,
    sync::{ipc_common::*, wait_list::WaitMode},
    thread::{SuspendFlag, Thread},
    timer::TimerControlAction,
};
use blue_infra::list::doubly_linked_list::LinkedListNode;

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
    raw: UnsafeCell<Mutex>,
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
        fn init_raw() -> impl PinInit<UnsafeCell<Mutex>> {
            let init = |slot: *mut UnsafeCell<Mutex>| {
                let slot: *mut Mutex = slot.cast();
                unsafe {
                    let cur_ref = &mut *slot;

                    if let Ok(s) = CString::try_from_fmt(fmt!("{:p}", slot)) {
                        cur_ref.init(s.as_ptr() as *const i8);
                    } else {
                        let default = "default";
                        cur_ref.init(default.as_ptr() as *const i8);
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
            let _ = (*self.raw.get()).lock();
        };
        KMutexGuard { mtx: self }
    }
}

pub struct KMutexGuard<'a, T> {
    mtx: &'a KMutex<T>,
}

impl<'a, T> Drop for KMutexGuard<'a, T> {
    fn drop(&mut self) {
        unsafe {
            let _ = (*self.mtx.raw.get()).unlock();
        }
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
pub struct Mutex {
    /// Inherit from KObjectBase
    #[pin]
    pub(crate) parent: KObjectBase,
    /// Priority ceiling of mutex
    pub(crate) ceiling_priority: u8,
    /// Maximal priority for pending thread
    pub(crate) priority: u8,
    /// Current owner of mutex
    pub(crate) owner: *mut Thread,
    /// The object list taken by thread
    #[pin]
    pub(crate) taken_node: LinkedListNode,
    /// SysQueue for mutex
    #[pin]
    pub(crate) inner_queue: SysQueue,
}

impl_kobject!(Mutex);

impl Mutex {
    #[inline]
    pub fn new(name: [i8; NAME_MAX]) -> impl PinInit<Self> {
        crate::debug_not_in_interrupt!();

        let init = move |slot: *mut Self| unsafe {
            let cur_ref = &mut *slot;
            let _ = KObjectBase::new(ObjectClassType::ObjectClassMutex as u8, name)
                .__pinned_init(&mut cur_ref.parent as *mut KObjectBase);

            cur_ref.owner = null_mut();
            cur_ref.priority = 0xFF;
            cur_ref.ceiling_priority = 0xFF;
            let _ =
                LinkedListNode::new().__pinned_init(&mut cur_ref.taken_node as *mut LinkedListNode);

            let _ = SysQueue::new(
                mem::size_of::<u32>(),
                1,
                IPC_SYS_QUEUE_STUB,
                WaitMode::Priority,
            )
            .__pinned_init(&mut cur_ref.inner_queue as *mut SysQueue);
            Ok(())
        };
        unsafe { pin_init_from_closure(init) }
    }
    #[inline]
    pub fn init(&mut self, name: *const i8) {
        // Flag can only be WaitMode::Priority.
        self.parent
            .init(ObjectClassType::ObjectClassMutex as u8, name);

        self.init_internal();
    }

    #[inline]
    pub fn init_dyn(&mut self, name: *const i8) {
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
            let _ = LinkedListNode::new().__pinned_init(&mut self.taken_node);
        }

        let _ = unsafe {
            SysQueue::new(
                mem::size_of::<u32>(),
                1,
                IPC_SYS_QUEUE_STUB,
                WaitMode::Priority,
            )
            .__pinned_init(&mut self.inner_queue as *mut SysQueue)
        };
    }

    #[inline]
    pub fn detach(&mut self) {
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassMutex as u8);

        // Critical section
        {
            let _ = self.inner_queue.spinlock.acquire();
            self.inner_queue.enqueue_waiter.wake_all();

            unsafe {
                Pin::new_unchecked(&mut self.taken_node).remove_from_list();
            }
        }
        Cpu::get_current_scheduler().do_task_schedule();

        if self.is_static_kobject() {
            self.parent.detach();
        }
    }

    #[inline]
    pub fn new_raw(name: *const i8) -> *mut Self {
        let mutex = Box::pin_init(Mutex::new(char_ptr_to_array(name)));
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

        // Critical section
        {
            let _ = self.inner_queue.spinlock.acquire();
            self.inner_queue.enqueue_waiter.wake_all();

            unsafe {
                Pin::new_unchecked(&mut self.taken_node).remove_from_list();
            }
        }

        self.parent.delete();
    }

    ///
    /// Mutex lock operation with concurrency control, priority management,
    /// and kernel resource synchronization.
    ///
    pub fn lock_internal(&mut self, timeout: i32, pending_mode: SuspendFlag) -> Result<(), Error> {
        // Shadowing a mutable timeout for input one
        let mut timeout = timeout;

        // Enable debug scheduler availability checks (likely for debugging purposes).
        crate::debug_scheduler_available!(true);

        // Assert that the object type is `ObjectClassMutex` by checking its type name.
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassMutex as u8);

        let thread_ptr = current_thread_ptr!();
        assert!(!thread_ptr.is_null());
        let thread = unsafe { &mut *thread_ptr };

        self.inner_queue.lock();

        thread.error = code::EOK;

        // Check if the current thread already owns the mutex.
        if self.owner == thread_ptr {
            // If the thread already owns the mutex, check if the recursive lock count is below the maximum allowed.
            if self.inner_queue.count() < IPC_MUTEX_NESTED_MAX as usize {
                // Increment the recursive lock count by pushing a stub into the inner queue.
                self.inner_queue.force_push_stub();
            } else {
                // If the recursive lock count exceeds the maximum, unlock the inner queue and return an error (`ENOSPC`).
                self.inner_queue.unlock();
                return Err(code::ENOSPC);
            }
        } else {
            // Case where mutex is not owned by current thread
            // Check if mutex is already owned by another thread
            if self.owner.is_null() {
                // Take ownership of the mutex
                self.owner = thread_ptr;
                // Reset priority tracking
                self.priority = 0xff;
                // Initialize lock count of the same thread
                self.inner_queue.reset_stub(1);
                let mutex_owner = unsafe { &mut *self.owner };

                // Handle priority ceiling protocol, non-0xFF means priority ceiling has been set
                if self.ceiling_priority != 0xFF {
                    // Elevate owner's priority to ceiling if needed
                    // This operation avoids the mutex taken thread at too low level priority which might be taking the
                    // mutex for too long time
                    if self.ceiling_priority < mutex_owner.priority.get_current() {
                        let _ = mutex_owner.update_priority(self.ceiling_priority, pending_mode);
                    }
                }

                // SAFETY: list should be ensured safe
                unsafe {
                    // Link mutex into thread's ownership list
                    Pin::new_unchecked(&mut thread.mutex_info.taken_list)
                        .push_front(Pin::new_unchecked(&mut self.taken_node));
                }
            } else {
                // Mutex is owned by another thread - handle contention
                // Immediate return for non-blocking case
                if timeout == 0 {
                    thread.error = code::ETIMEDOUT;
                    self.inner_queue.unlock();
                    return Err(code::ETIMEDOUT);
                } else {
                    let mut priority = thread.priority.get_current();
                    // Add thread to wait queue to be suspended
                    self.inner_queue
                        .enqueue_waiter
                        .wait(thread, pending_mode)
                        .map_err(|e| {
                            self.inner_queue.unlock();
                            e
                        })?;

                    // Set thread's pending mutex reference
                    thread.mutex_info.pending_to = unsafe { Some(NonNull::new_unchecked(self)) };

                    // Update mutex priority with waiting thread's priority
                    if priority < self.priority {
                        self.priority = priority;
                        let mutex_owner = unsafe { &mut *self.owner };

                        // Priority inheritance
                        if self.priority < mutex_owner.priority.get_current() {
                            let _ =
                                mutex_owner.update_priority(priority, SuspendFlag::Uninterruptible);
                        }
                    }

                    // Set timeout for timer if specified, and start timer to wait
                    if timeout > 0 {
                        thread.thread_timer.timer_control(
                            TimerControlAction::SetTime,
                            (&mut timeout) as *mut i32 as *mut ffi::c_void,
                        );

                        thread.thread_timer.start();
                    }

                    self.inner_queue.unlock();

                    // Do scheduling
                    Cpu::get_current_scheduler().do_task_schedule();

                    self.inner_queue.lock();

                    // Mutex not taken
                    if thread.error != code::EOK {
                        // Handle error case - clean up and adjust priorities
                        let mut need_update = false;

                        unsafe {
                            // Check if owner priority needs adjustment
                            // The mutex owner might be part of a priority inheritance chain
                            // Even with equal priorities, the owner's priority might need propagation through others
                            // Ensures proper scheduling when multiple threads have matching priorities
                            if !self.owner.is_null()
                                && (*self.owner).priority.get_current()
                                    == thread.priority.get_current()
                            {
                                need_update = true;
                            }

                            // Updates the mutex's recorded priority based on waiting threads:
                            // Takes priority from the first waiter if queue isn't empty
                            // Mutex priority ceiling protocol control
                            // Maintains the highest priority among waiters for proper priority inheritance
                            if let Some(node) = self.inner_queue.enqueue_waiter.head() {
                                let th =
                                    crate::thread_list_node_entry!(node.as_ptr()) as *mut Thread;
                                self.priority = (*th).priority.get_current();
                            } else {
                                self.priority = 0xff;
                            }
                        }

                        // Try to change the priority of mutex owner if necessary
                        // Proper scheduling when thread priorities dynamically change
                        if need_update {
                            // SAFETY: self owner is not null
                            let mutex_owner = unsafe { &mut *self.owner };
                            priority = mutex_owner.get_mutex_priority();

                            if priority != mutex_owner.priority.get_current() {
                                let _ = mutex_owner
                                    .update_priority(priority, SuspendFlag::Uninterruptible);
                            }
                        }

                        self.inner_queue.unlock();

                        // Ownership set, pending none
                        thread.mutex_info.pending_to = None;
                        if thread.error != code::EOK {
                            return Err(thread.error);
                        }
                        return Ok(());
                    }
                }
            }
        }

        self.inner_queue.unlock();

        Ok(())
    }

    pub fn lock(&mut self) -> Result<(), Error> {
        self.lock_internal(WAITING_FOREVER as i32, SuspendFlag::Uninterruptible)
    }

    pub fn lock_wait(&mut self, time: i32) -> Result<(), Error> {
        self.lock_internal(time, SuspendFlag::Uninterruptible)
    }

    pub fn try_lock(&mut self) -> Result<(), Error> {
        self.lock_wait(WAITING_NO as i32)
    }

    ///
    /// Mutex unlock operation, handling ownership transfer, priority adjustments, and waking waiters.
    ///
    pub fn unlock(&mut self) -> Result<(), Error> {
        // Verify this is actually a mutex object
        assert_eq!(self.type_name(), ObjectClassType::ObjectClassMutex as u8);

        // Mutex release must happen in thread context due to ownership checks
        crate::debug_in_thread_context!();

        let thread_ptr = current_thread_ptr!();
        if thread_ptr.is_null() {
            return Err(code::ERROR);
        }
        let thread = unsafe { &mut *thread_ptr };
        let mut need_schedule = false;

        // Critical section
        {
            let _ = self.inner_queue.spinlock.acquire();

            // Verify current thread actually owns the mutex
            if thread_ptr != self.owner {
                thread.error = code::ERROR;
                return Err(code::ERROR);
            }

            // Decrement recursive lock count by popping stub
            self.inner_queue.pop_stub();

            // Check if this was the last recursive lock
            if self.inner_queue.is_empty() {
                // Remove mutex from current thread's ownership list
                unsafe {
                    Pin::new_unchecked(&mut self.taken_node).remove_from_list();
                }
                // Handle priority adjustments if:
                // - Using priority ceiling protocol OR
                // - Thread's current priority matches mutex priority, priority inheritance chain reverting
                if self.ceiling_priority != 0xFF || thread.priority.get_current() == self.priority {
                    let priority = thread.get_mutex_priority();

                    // Update thread priority if changed
                    thread.change_priority(priority);

                    need_schedule = true;
                }

                // Takes priority from the first waiter if queue isn't empty
                if !self.inner_queue.enqueue_waiter.is_empty() {
                    #[allow(unused_assignments)]
                    let mut next_thread_ptr: *mut Thread = null_mut();

                    if let Some(node) = self.inner_queue.enqueue_waiter.head() {
                        next_thread_ptr = unsafe { crate::thread_list_node_entry!(node.as_ptr()) };
                        if next_thread_ptr.is_null() {
                            return Err(code::ERROR);
                        }
                    } else {
                        return Err(code::ERROR);
                    }

                    let next_thread = unsafe { &mut *next_thread_ptr };

                    unsafe { Pin::new_unchecked(&mut next_thread.list_node).remove_from_list() };

                    // Transfer ownership to next thread
                    self.owner = next_thread_ptr;
                    self.inner_queue.reset_stub(1); // Reset lock count for new owner

                    // Add mutex to new owner's taken list
                    unsafe {
                        Pin::new_unchecked(&mut next_thread.mutex_info.taken_list)
                            .push_front(Pin::new_unchecked(&mut self.taken_node));
                    }
                    // Clear pending status and resume new owner
                    next_thread.mutex_info.pending_to = None;
                    next_thread.resume();

                    // Unlock only release one waiter, if there is still any, update mutex priority to its
                    // Update mutex priority based on new waiters (if any)
                    if !self.inner_queue.enqueue_waiter.is_empty() {
                        #[allow(unused_assignments)]
                        let mut th: *mut Thread = null_mut();
                        if let Some(node) = self.inner_queue.enqueue_waiter.head() {
                            th = unsafe { crate::thread_list_node_entry!(node.as_ptr()) };
                            if th.is_null() {
                                return Err(code::ERROR);
                            }
                        } else {
                            return Err(code::ERROR);
                        }

                        // Set mutex priority to highest waiting thread's priority
                        unsafe {
                            self.priority = (*th).priority.get_current();
                        }
                    } else {
                        // No waiters remaining, reset priority
                        self.priority = 0xff;
                    }

                    need_schedule = true;
                } else {
                    // No waiters - clear ownership and reset priority
                    self.owner = null_mut();
                    self.priority = 0xff;
                }
            }
        }

        // Schedule if priority changed or ownership transferred
        if need_schedule {
            Cpu::get_current_scheduler().do_task_schedule();
        }

        Ok(())
    }

    #[inline]
    pub(crate) fn update_priority(&mut self) -> u8 {
        // Check if there is a thread waiting in the queue
        if let Some(node) = self.inner_queue.enqueue_waiter.head() {
            // If a thread is waiting, get its priority
            unsafe {
                let thread: *mut Thread = crate::thread_list_node_entry!(node.as_ptr());
                // Retrieve the current priority of the thread
                self.priority = (*thread).priority.get_current();
            }
        } else {
            // If no thread is waiting, set the priority to the maximum value (0xff)
            self.priority = 0xff;
        }

        // Return the updated priority
        self.priority
    }

    pub(crate) fn drop_thread(&mut self, thread_ptr: *mut Thread) {
        // Early return if the thread pointer is null to avoid unsafe operations on null.
        if thread_ptr.is_null() {
            return;
        }
        // SAFETY: thread is null checked
        let thread = unsafe { &mut *thread_ptr };

        // Flag to track if the mutex owner's priority needs to be updated.
        let mut need_update = false;

        // Remove the thread from the thread list to clean up its state.
        thread.remove_thread_list_node();

        // If the mutex has no owner, there's nothing more to do.
        if self.owner.is_null() {
            return;
        }

        // SAFETY: The owner is null-checked above, so it is safe to dereference.
        let mutex_owner = unsafe { &mut *self.owner };

        // Check if the thread being dropped has the same priority as the mutex owner.
        // If so, the owner's priority might need to be updated.
        if mutex_owner.priority.get_current() == thread.priority.get_current() {
            need_update = true;
        }

        if let Some(node) = self.inner_queue.enqueue_waiter.head() {
            unsafe {
                let th: *mut Thread = crate::thread_list_node_entry!(node.as_ptr());
                self.priority = (*th).priority.get_current();
            }
        } else {
            self.priority = 0xff;
        }

        if need_update {
            let priority = mutex_owner.get_mutex_priority();
            if priority != mutex_owner.priority.get_current() {
                let _ = mutex_owner.update_priority(priority, SuspendFlag::Uninterruptible);
            }
        }
    }

    pub(crate) fn set_prio_ceiling(&mut self, priority: u8) -> u8 {
        let mut prev_priority: u8 = 0xFF;

        if priority < THREAD_PRIORITY_MAX as u8 {
            //Critical section
            {
                let _ = self.inner_queue.spinlock.acquire();
                prev_priority = self.ceiling_priority;
                self.ceiling_priority = priority;
                let owner_thread = self.owner;
                if !owner_thread.is_null() {
                    // SAFETY: owner_thread is null checked, so it is safe to dereference.
                    unsafe {
                        // If the calculated priority differs from the current priority of the owner thread,
                        // update the owner thread's priority to reflect the new mutex priority.
                        let priority = (*owner_thread).get_mutex_priority();

                        if priority != (*owner_thread).priority.get_current() {
                            let _ = (*owner_thread)
                                .update_priority(priority, SuspendFlag::Uninterruptible);
                        }
                    }
                }
            }
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

pub struct RawMutexGuard<'a> {
    mutex: &'a mut Mutex,
}

impl<'a> Drop for RawMutexGuard<'a> {
    fn drop(&mut self) {
        let _ = self.mutex.unlock();
    }
}

impl Mutex {
    pub fn acquire(&mut self) -> Result<RawMutexGuard, Error> {
        self.lock()?;
        Ok(RawMutexGuard { mutex: self })
    }
}
