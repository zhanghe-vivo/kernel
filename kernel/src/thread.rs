#![allow(dead_code)]
#[cfg(smp)]
use crate::cpu::CPU_DETACHED;
use crate::{
    allocator::malloc,
    arch::Arch,
    clock,
    cpu::Cpu,
    error::{code, Error},
    object::{KObjectBase, KernelObject, ObjectClassType},
    stack::Stack,
    static_init::UnsafeStaticInit,
    sync::{lock::mutex::*, RawSpin, SpinLock},
    timer::{Timer, TimerState},
    zombie,
};
use bluekernel_infra::list::doubly_linked_list::{LinkedListNode, ListHead};
use core::{
    cell::UnsafeCell,
    ffi,
    marker::PhantomPinned,
    mem,
    pin::Pin,
    ptr::{self, NonNull},
    sync::atomic::{AtomicUsize, Ordering},
};
use log::debug;
use pinned_init::{
    init, pin_data, pin_init, pin_init_from_closure, pinned_drop, zeroed, Init, PinInit,
};

// compatible with C
pub type ThreadEntryFn = extern "C" fn(*mut ffi::c_void);
pub type ThreadCleanupFn = extern "C" fn(*mut Thread);

pub(crate) const THREAD_DEFAULT_TICK: u32 = 50;

/// Returns the currently running thread.
#[macro_export]
macro_rules! current_thread {
    () => {
        $crate::cpu::Cpu::get_current_scheduler().get_current_thread()
    };
}
pub use current_thread;

#[macro_export]
macro_rules! current_thread_ptr {
    () => {
        unsafe {
            if let Some(mut curth) = $crate::current_thread!() {
                curth.as_mut()
            } else {
                null_mut()
            }
        }
    };
}
pub use current_thread_ptr;

/// Returns the currently running thread.
#[macro_export]
macro_rules! current_thread_mut {
    () => {
        &mut *$crate::cpu::Cpu::get_current_scheduler()
            .get_current_thread()
            .unwrap()
            .as_mut()
    };
}
pub use current_thread_mut;

#[macro_export]
macro_rules! thread_list_node_entry {
    ($node:expr) => {
        crate::container_of!($node, crate::thread::Thread, list_node)
    };
}
pub use thread_list_node_entry;

const MAX_THREAD_SIZE: usize = 1024;
pub(crate) static TIDS: SpinLock<[Option<usize>; MAX_THREAD_SIZE]> =
    SpinLock::new([const { None }; MAX_THREAD_SIZE]);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SuspendFlag {
    Interruptible = 0,
    Killable = 1,
    Uninterruptible = 2,
}

impl SuspendFlag {
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Interruptible,
            1 => Self::Killable,
            2 => Self::Uninterruptible,
            _ => Self::Interruptible,
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThreadState(u8);

impl ThreadState {
    pub const INIT: Self = Self(0b0000_0000);
    pub const CLOSE: Self = Self(0b0000_0001);
    pub const READY: Self = Self(0b0000_0010);
    pub const RUNNING: Self = Self(0b0000_0011);
    pub const SUSPENDED: Self = Self(0b0000_0100);
    pub const STATE_MASK: Self = Self(0b0000_0111);
    pub const YIELD: Self = Self(0b0000_1000);

    pub const SUSPENDED_INTERRUPTIBLE: Self = Self(Self::SUSPENDED.0);
    pub const SUSPENDED_KILLABLE: Self = Self(Self::SUSPENDED.0 | 0b0000_0001);
    pub const SUSPENDED_UNINTERRUPTIBLE: Self = Self(Self::SUSPENDED.0 | 0b0000_0010);

    fn base_state(self) -> Self {
        Self(self.0 & Self::STATE_MASK.0)
    }

    pub fn is_init(self) -> bool {
        self.base_state() == Self::INIT
    }

    pub fn is_close(self) -> bool {
        self.base_state() == Self::CLOSE
    }

    pub fn is_ready(self) -> bool {
        self.base_state() == Self::READY
    }

    pub fn is_running(self) -> bool {
        self.base_state() == Self::RUNNING
    }

    pub fn is_suspended(self) -> bool {
        (self.0 & Self::SUSPENDED.0) != 0
    }

    pub fn get_suspend_flag(self) -> SuspendFlag {
        SuspendFlag::from_u8(self.0 & 0b0000_0011)
    }

    pub fn is_yield(self) -> bool {
        (self.0 & Self::YIELD.0) != 0
    }

    pub fn set_base_state(&mut self, state: Self) {
        self.0 = state.0;
    }

    pub fn set_suspended(&mut self, flag: SuspendFlag) {
        self.0 = Self::SUSPENDED.0 | (flag as u8);
    }

    pub fn add_yield(&mut self) {
        self.0 |= Self::YIELD.0;
    }

    pub fn clear_yield(&mut self) {
        self.0 &= !Self::YIELD.0;
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ThreadPriority {
    current: u8,
    initial: u8,
    #[cfg(thread_priority)]
    number: u8,
    #[cfg(thread_priority)]
    high_mask: u8,
    number_mask: u32,
}

impl ThreadPriority {
    pub fn new(priority: u8) -> Self {
        #[cfg(thread_priority)]
        {
            let number = priority >> 3;
            let high_mask = 1 << (priority & 0x07);
            let number_mask = 1 << number;
            Self {
                current: priority,
                initial: priority,
                number,
                high_mask,
                number_mask,
            }
        }
        #[cfg(not(thread_priority))]
        {
            Self {
                current: priority,
                initial: priority,
                number_mask: 1 << priority,
            }
        }
    }

    pub fn update(&mut self, new_priority: u8) {
        self.current = new_priority;
        #[cfg(thread_priority)]
        {
            self.number = new_priority >> 3;
            self.high_mask = 1 << (new_priority & 0x07);
            self.number_mask = 1 << self.number;
        }
        #[cfg(not(thread_priority))]
        {
            self.number_mask = 1 << new_priority;
        }
    }

    pub fn get_current(&self) -> u8 {
        self.current
    }

    pub fn get_initial(&self) -> u8 {
        self.initial
    }

    #[cfg(thread_priority)]
    #[inline]
    pub fn get_number(&self) -> u8 {
        self.number
    }

    #[cfg(thread_priority)]
    #[inline]
    pub fn get_high_mask(&self) -> u8 {
        self.high_mask
    }

    #[inline]
    pub fn get_number_mask(&self) -> u32 {
        self.number_mask
    }
}

#[cfg(schedule_with_time_slice)]
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct TimeSlice {
    pub init: u32,
    pub remaining: u32,
}

// NOTE: pin_data conflicts with cfg.
#[cfg(mutex)]
#[repr(C)]
#[pin_data]
pub(crate) struct MutexInfo {
    #[pin]
    pub(crate) taken_list: ListHead,
    pub(crate) pending_to: Option<NonNull<Mutex>>,
}

#[cfg(not(mutex))]
#[repr(C)]
pub(crate) struct MutexInfo {}

#[cfg(mutex)]
impl MutexInfo {
    fn new() -> impl PinInit<Self> {
        pin_init!(Self {
            taken_list <- ListHead::new(),
            pending_to : None,
        })
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct EventInfo {
    #[cfg(event)]
    pub(crate) set: u32,
    #[cfg(event)]
    pub(crate) info: u8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct CpuAffinity {
    #[cfg(smp)]
    pub bind_cpu: u8,
    #[cfg(smp)]
    pub oncpu: u8,
}

#[repr(C)]
#[derive(Debug)]
struct LockInfo {
    #[cfg(debugging_spinlock)]
    pub wait_lock: Option<NonNull<RawSpin>>,
    #[cfg(debugging_spinlock)]
    pub hold_locks: [Option<NonNull<RawSpin>>; 8],
    #[cfg(debugging_spinlock)]
    pub hold_count: usize,
}

/// cbindgen:field-names=[parent, tlist]
#[repr(C)]
#[pin_data(PinnedDrop)]
pub struct Thread {
    #[pin]
    pub parent: KObjectBase,

    // thread linked list, link to priority_table or pending list
    #[pin]
    pub(crate) list_node: LinkedListNode,

    /// thread status
    pub stat: ThreadState,

    /// priority manager
    pub(crate) priority: ThreadPriority,

    /// stack point and cleanup func
    pub(crate) stack: Stack,

    // Can't add Option because of cbindgen.
    pub cleanup: ThreadCleanupFn,

    tid: usize,

    /// built-in thread timer, used for wait timeout
    #[pin]
    pub(crate) thread_timer: Timer,

    spinlock: RawSpin,

    /// error code
    pub error: Error,

    /// time slice
    #[cfg(schedule_with_time_slice)]
    time_slice: TimeSlice,

    #[cfg(mutex)]
    pub(crate) mutex_info: MutexInfo,

    #[cfg(event)]
    pub(crate) event_info: EventInfo,

    /// cpu affinity
    #[cfg(smp)]
    cpu_affinity: CpuAffinity,

    #[cfg(debugging_spinlock)]
    lock_info: LockInfo,

    /// Indicate whether the stack should be free'ed when this thread
    /// goes to end of life. This usually happens when Thread's user
    /// doesn't specify stack_start and Thread's ctor allocates a stack
    /// for the user.
    should_free_stack: bool,

    #[pin]
    pin: PhantomPinned,
}

#[repr(C, align(8))]
struct StackAlignedField<const STACK_SIZE: usize> {
    buf: [u8; STACK_SIZE],
}

#[pin_data]
pub struct ThreadWithStack<const STACK_SIZE: usize> {
    #[pin]
    pub(crate) inner: UnsafeCell<Thread>,
    #[pin]
    stack: StackAlignedField<STACK_SIZE>,
}

impl<const STACK_SIZE: usize> StackAlignedField<STACK_SIZE> {
    #[inline]
    pub fn new() -> impl Init<Self> {
        init!(StackAlignedField { buf <- zeroed() })
    }
    #[inline]
    pub const fn size(&self) -> usize {
        STACK_SIZE
    }
    #[inline]
    pub const fn get_buf_ptr(&self) -> *mut u8 {
        &self.buf as *const _ as *mut u8
    }
}

impl<const STACK_SIZE: usize> ThreadWithStack<STACK_SIZE> {
    #[inline]
    pub fn new(
        name: &'static ffi::CStr,
        entry: ThreadEntryFn,
        parameter: *mut usize,
        priority: u8,
        tick: u32,
    ) -> impl PinInit<Self> {
        pin_init!(&this in Self {
            stack <- StackAlignedField::<STACK_SIZE>::new(),
            inner <- unsafe { pin_init_from_closure(move |slot: *mut UnsafeCell<Thread>| {
                let stack_addr = this.as_ref().stack.get_buf_ptr();
                let init = ThreadBuilder::default().stack_start(stack_addr)
                                        .stack_size(STACK_SIZE)
                                        .entry_fn(entry)
                                        .arg(parameter as *mut ffi::c_void)
                                        .priority(priority)
                                        .tick(tick)
                                        .name(name).build_pinned_init();
                init.__pinned_init(slot.cast::<Thread>())
            })},
        })
    }

    #[cfg(smp)]
    #[inline]
    pub fn new_with_bind(
        name: &'static ffi::CStr,
        entry: ThreadEntryFn,
        parameter: *mut usize,
        priority: u8,
        tick: u32,
        cpu: u8,
    ) -> impl PinInit<Self> {
        pin_init!(&this in Self {
            stack <- StackAlignedField::<STACK_SIZE>::new(),
            inner <- unsafe { pin_init_from_closure(move |slot: *mut UnsafeCell<Thread>| {
                    let stack_addr = this.as_ref().stack.get_buf_ptr();
                    let init = ThreadBuilder::default().stack_start(stack_addr)
                        .stack_size(STACK_SIZE)
                        .entry_fn(entry)
                        .args(parameter as *mut ffi::c_void)
                        .priority(priority)
                        .tick(tick)
                        .cpu(cpu)
                        .name(name).build_pinned_init();
                    init.__pinned_init(slot.cast::<Thread>())
                }
            )},
        })
    }
}

impl<const STACK_SIZE: usize> core::ops::Deref for ThreadWithStack<STACK_SIZE> {
    type Target = Thread;

    fn deref(&self) -> &Self::Target {
        // SAFETY: The caller owns the lock, so it is safe to deref the protected data.
        unsafe { &*self.inner.get() }
    }
}

impl<const STACK_SIZE: usize> core::ops::DerefMut for ThreadWithStack<STACK_SIZE> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: The caller owns the lock, so it is safe to deref the protected data.
        unsafe { &mut *self.inner.get() }
    }
}

impl Thread {
    // Used for hw_context_switch.
    #[inline]
    pub(crate) fn sp_ptr(&self) -> *const usize {
        self.stack.sp_ptr()
    }

    pub(crate) fn stack(&self) -> &Stack {
        &self.stack
    }
    pub(crate) fn stack_mut(&mut self) -> &mut Stack {
        &mut self.stack
    }

    #[inline]
    pub fn reset_to_yield(&mut self) {
        debug_assert!(Cpu::get_current_scheduler().is_sched_locked());

        #[cfg(schedule_with_time_slice)]
        {
            self.time_slice.remaining = self.time_slice.init;
        }
        self.stat.add_yield();
    }

    #[cfg(smp)]
    #[inline]
    pub fn is_cpu_detached(&self) -> bool {
        self.oncpu == CPU_DETACHED as u8
    }

    #[cfg(smp)]
    #[inline]
    pub fn is_bind_cpu(&self) -> bool {
        self.bind_cpu != CPUS_NUMBER as u8
    }

    #[cfg(smp)]
    #[inline]
    pub fn set_bind_cpu(&mut self, cpu_id: u8) {
        debug_assert!(cpu_id < CPUS_NUMBER as u8);
        self.bind_cpu = cpu_id;
    }

    #[cfg(smp)]
    #[inline]
    pub fn get_bind_cpu(&self) -> u8 {
        self.bind_cpu
    }

    #[inline]
    pub fn get_cleanup_fn(&self) -> ThreadCleanupFn {
        self.cleanup
    }

    #[inline]
    pub fn is_current_runnung_thread(&self) -> bool {
        ptr::eq(
            self,
            Cpu::get_current_scheduler()
                .current_thread
                .load(Ordering::Acquire),
        )
    }

    #[inline]
    pub(crate) fn get_name(&self) -> &ffi::CStr {
        unsafe { ffi::CStr::from_ptr(self.name()) }
    }

    #[inline]
    pub(crate) fn remove_thread_list_node(&mut self) {
        unsafe { Pin::new_unchecked(&mut self.list_node).remove_from_list() };
    }

    pub(crate) fn new_tid(thread_ptr: usize) -> usize {
        static TID: AtomicUsize = AtomicUsize::new(0);
        let id = TID.fetch_add(1, Ordering::SeqCst);
        let mut tids = TIDS.lock();
        if id >= MAX_THREAD_SIZE || !tids[id].is_none() {
            for i in 0..MAX_THREAD_SIZE {
                if tids[i].is_none() {
                    tids[i] = Some(thread_ptr);
                    TID.store(0, Ordering::SeqCst);
                    return i;
                }
            }
            panic!("The maximum number of threads has been exceeded");
        }
        id
    }

    #[no_mangle]
    extern "C" fn default_cleanup(_thread: *mut Thread) {}

    /// Handler for thread timeout.
    #[no_mangle]
    extern "C" fn handle_timeout(para: *mut ffi::c_void) {
        debug_assert!(para != ptr::null_mut());

        let thread = unsafe { &mut *(para as *mut Thread) };
        debug_assert!(thread.type_name() == ObjectClassType::ObjectClassThread);

        let scheduler = Cpu::get_current_scheduler();
        let level = scheduler.sched_lock();

        debug_assert!(thread.stat.is_suspended());
        thread.error = code::ETIMEDOUT;
        thread.remove_thread_list_node();
        scheduler.insert_thread_locked(thread);

        scheduler.sched_unlock_with_sched(level);
    }

    pub(crate) unsafe extern "C" fn exit() {
        debug_assert!(Arch::is_interrupts_active());
        let th = crate::current_thread!().unwrap().as_mut();
        th.detach();
        debug_assert!(Arch::is_interrupts_active());
        panic!("!!! never get here !!!, thread {:?}", th as *const Thread);
    }

    #[inline]
    pub fn handle_tick_increase(&mut self) -> bool {
        debug_assert!(Cpu::get_current_scheduler().is_sched_locked());
        debug_assert!(self.is_current_runnung_thread());
        self.time_slice.remaining -= 1;
        if self.time_slice.remaining == 0 {
            self.reset_to_yield();
            return true;
        }
        false
    }

    #[inline]
    fn detach_from_mutex(&mut self) {
        let level = self.spinlock.lock_irqsave();

        // Releasing raw mutex might use sched_lock.
        if let Some(mut pending_mutex) = self.mutex_info.pending_to {
            unsafe {
                pending_mutex.as_mut().drop_thread(self);
            }
            self.mutex_info.pending_to = None;
        }

        crate::doubly_linked_list_for_each!(node, &self.mutex_info.taken_list, {
            unsafe {
                let mutex = crate::list_head_entry!(node.as_ptr(), Mutex, taken_node) as *mut Mutex;
                if !mutex.is_null() {
                    // as mutex will be removed from list, so we need to get prev node
                    node = node.prev().unwrap_unchecked().as_ref();
                    let _ = (*mutex).unlock();
                }
            }
        });

        self.spinlock.unlock_irqrestore(level);
    }

    #[inline]
    pub(crate) fn get_mutex_priority(&self) -> u8 {
        let mut priority = self.priority.get_initial();

        crate::doubly_linked_list_for_each!(node, &self.mutex_info.taken_list, {
            let mutex = unsafe { &*crate::list_head_entry!(node.as_ptr(), Mutex, taken_node) };
            let mut mutex_prio = mutex.priority;
            mutex_prio = if mutex_prio < mutex.ceiling_priority {
                mutex_prio
            } else {
                mutex.ceiling_priority
            };

            if priority > mutex_prio {
                priority = mutex_prio;
            }
        });

        priority
    }

    #[inline]
    pub(crate) fn update_priority(
        &mut self,
        priority: u8,
        suspend_flag: SuspendFlag,
    ) -> Result<(), Error> {
        // Change priority of the thread.
        self.change_priority(priority);
        while self.stat.is_suspended() {
            // Whether change the priority of the taken mutex.
            if let Some(mut pending_mutex) = self.mutex_info.pending_to {
                let pending_mutex = unsafe { pending_mutex.as_mut() };
                let owner_thread = unsafe { &mut *pending_mutex.owner };
                // Re-insert thread to suspended thread list.
                self.remove_thread_list_node();

                match pending_mutex
                    .inner_queue
                    .enqueue_waiter
                    .wait(self, suspend_flag)
                {
                    Ok(_) => {
                        pending_mutex.update_priority();
                        let mutex_priority = owner_thread.get_mutex_priority();
                        if mutex_priority != owner_thread.priority.get_current() {
                            owner_thread.change_priority(mutex_priority);
                        } else {
                            return Err(code::ERROR);
                        }
                    }
                    Err(e) => {
                        return Err(e);
                    }
                }
            }
        }

        Ok(())
    }

    pub fn start(&mut self) {
        let scheduler = Cpu::get_current_scheduler();
        let level = scheduler.sched_lock();

        #[cfg(debugging_scheduler)]
        debug!("Thread {:?} is starting...", self as *const Self);

        self.priority.update(self.priority.get_current());
        self.stat.set_suspended(SuspendFlag::Uninterruptible);
        if scheduler.insert_ready_locked(self) {
            scheduler.sched_unlock_with_sched(level);
        } else {
            scheduler.sched_unlock(level);
        }
    }

    pub fn close(&mut self) {
        // assert!(!self.is_current_runnung_thread());
        let scheduler = Cpu::get_current_scheduler();
        let level = scheduler.sched_lock();

        #[cfg(debugging_scheduler)]
        debug!("Thread {:?} is closing...", self as *const Self);

        if !self.stat.is_close() {
            if !self.stat.is_init() {
                scheduler.remove_thread_locked(self);
            }
            self.thread_timer.detach();
            self.stat.set_base_state(ThreadState::CLOSE);
        }

        scheduler.sched_unlock(level);
    }

    pub fn detach(&mut self) {
        // Forbid scheduling on current core before returning since current thread
        // may be detached from scheduler.
        let scheduler = Cpu::get_current_scheduler();
        scheduler.preempt_disable();
        TIDS.lock()[self.tid as usize] = None;
        #[cfg(debugging_scheduler)]
        debug!("Thread {:?} is detaching...", self as *const Self);

        self.close();

        #[cfg(mutex)]
        self.detach_from_mutex();

        unsafe {
            (&raw const zombie::ZOMBIE_MANAGER as *const UnsafeStaticInit<zombie::ZombieManager, _>)
                .cast_mut()
                .as_mut()
                .unwrap_unchecked()
                .zombie_enqueue(self)
        };

        scheduler.do_task_schedule();
        scheduler.preempt_enable();
    }

    pub(crate) fn timer_stop(&mut self) -> bool {
        debug_assert!(Cpu::get_current_scheduler().is_sched_locked());
        self.thread_timer.stop()
    }

    pub fn yield_now() {
        let scheduler = Cpu::get_current_scheduler();
        scheduler.yield_now();
    }

    pub fn msleep(ms: u32) -> Result<(), Error> {
        let tick = clock::tick_from_millisecond(ms as i32);
        Self::sleep(tick)
    }

    pub fn sleep(tick: u32) -> Result<(), Error> {
        if tick == 0 {
            return Err(code::EINVAL);
        }

        let scheduler = Cpu::get_current_scheduler();
        if scheduler.is_sched_locked() {
            return Err(code::EBUSY);
        }

        scheduler.preempt_disable();

        let thread = unsafe { crate::current_thread!().unwrap().as_mut() };
        // Reset thread error.
        thread.error = code::EOK;

        #[cfg(debugging_scheduler)]
        debug!("Thread {:?} is sleeping...", thread as *const Thread);

        if thread.suspend(SuspendFlag::Interruptible) {
            thread.thread_timer.restart(tick);
            thread.error = code::EINTR;
            // Notify for a pending rescheduling.
            scheduler.do_task_schedule();
            // Exit critical region and do a rescheduling.
            scheduler.preempt_enable();

            if thread.error == code::ETIMEDOUT {
                thread.error = code::EOK;
            }
        }

        Ok(())
    }

    pub fn suspend(&mut self, suspend_flag: SuspendFlag) -> bool {
        assert!(self.is_current_runnung_thread());
        let scheduler = Cpu::get_current_scheduler();

        let level = scheduler.sched_lock();

        #[cfg(debugging_scheduler)]
        debug!("Thread {:?} is suspending...", self as *const Self);

        if (!self.stat.is_ready()) && (!self.stat.is_running()) {
            debug!("thread suspend: thread disorder, stat: {:?}", self.stat);
            scheduler.sched_unlock(level);
            return false;
        }

        scheduler.remove_thread_locked(self);
        #[cfg(smp)]
        {
            self.oncpu = CPU_DETACHED as u8;
        }
        self.stat.set_suspended(suspend_flag);
        self.timer_stop();
        scheduler.sched_unlock(level);

        return true;
    }

    pub fn resume(&mut self) -> bool {
        let scheduler = Cpu::get_current_scheduler();

        let level = scheduler.sched_lock();
        #[cfg(debugging_scheduler)]
        debug!("Thread {:?} is resuming...", self as *const Self);
        self.remove_thread_list_node();
        let need_schedule = scheduler.insert_ready_locked(self);
        scheduler.sched_unlock(level);

        return need_schedule;
    }

    pub fn change_priority(&mut self, priority: u8) {
        let scheduler = Cpu::get_current_scheduler();
        let level = scheduler.sched_lock();
        if self.stat.is_ready() {
            scheduler.remove_thread_locked(self);
            self.priority.update(priority);
            self.stat.set_base_state(ThreadState::INIT);
            scheduler.insert_thread_locked(self);
        } else {
            self.priority.update(priority);
        }
        scheduler.sched_unlock(level);
    }

    #[cfg(smp)]
    pub fn bind_to_cpu(&mut self, cpu: u8) {
        let cpu: u8 = if cpu >= CPUS_NUMBER as u8 {
            CPUS_NUMBER as u8
        } else {
            cpu
        };

        let scheduler = Cpu::get_current_scheduler();
        let level = scheduler.sched_lock();

        if self.stat.is_ready() {
            scheduler.remove_thread_locked(self);
            self.set_bind_cpu(cpu);
            scheduler.insert_thread_locked(self);
            scheduler.sched_unlock_with_sched(level);
        } else {
            self.bind_cpu = cpu;
            // Otherwise, the thread is running on a cpu.
            let current_cpu = scheduler.get_current_id();
            if cpu != CPUS_NUMBER as u8 {
                if cpu != current_cpu {
                    unsafe {
                        // TODO: call from libcpu.
                        // rt_bindings::rt_hw_ipi_send(rt_bindings::RT_SCHEDULE_IPI as i32, 1 << cpu)
                    };
                    // self cpu need reschedule
                    scheduler.sched_unlock_with_sched(level);
                }
            } else {
                // Not running on self cpu, but destintation cpu can be itself.
                unsafe {
                    // TODO: call from libcpu.
                    // rt_bindings::rt_hw_ipi_send(
                    //     rt_bindings::RT_SCHEDULE_IPI as i32,
                    //     1 << self.oncpu,
                    // )
                };
                scheduler.sched_unlock(level);
            }
        }
    }

    #[cfg(debugging_spinlock)]
    pub(crate) fn check_deadlock(&self, spin: &RawSpin) -> bool {
        let mut owner: Cell<Option<NonNull<Thread>>> = spin.owner.clone();
        while let Some(non_null) = owner.get() {
            let th = unsafe { non_null.as_ref() };
            if ptr::eq(self, th) {
                return true;
            }

            if let Some(wait_lock) = th.lock_info.wait_lock {
                owner = unsafe { wait_lock.as_ref().owner.clone() };
            } else {
                break;
            }
        }
        false
    }

    #[cfg(debugging_spinlock)]
    pub(crate) fn set_wait(&mut self, spin: &RawSpin) {
        self.wait_lock = Some(NonNull::new(spin as *const _ as *mut _));
    }

    #[cfg(debugging_spinlock)]
    pub(crate) fn clear_wait(&mut self) {
        self.lock_info.wait_lock = None;
    }

    pub(crate) fn should_free_stack(&self) -> bool {
        self.should_free_stack
    }

    pub(crate) fn set_should_free_stack(&mut self, flag: bool) {
        self.should_free_stack = flag;
    }

    pub(crate) fn tid(&self) -> usize {
        self.tid
    }
}

#[pinned_drop]
impl PinnedDrop for Thread {
    fn drop(self: Pin<&mut Self>) {
        let this_th = unsafe { Pin::get_unchecked_mut(self) };

        #[cfg(debugging_scheduler)]
        debug!("Dropping thread {:?}", this_th as *const Self);

        this_th.detach();
    }
}

crate::impl_kobject!(Thread);

/// bindgen for Thread
#[allow(improper_ctypes_definitions)]
#[no_mangle]
pub extern "C" fn bindgen_thread(_thread: Thread) {
    0;
}

pub struct ThreadBuilder {
    static_allocated: Option<NonNull<Thread>>,
    name: Option<&'static ffi::CStr>,
    entry_fn: Option<ThreadEntryFn>,
    arg: *mut core::ffi::c_void,
    cleanup_fn: Option<ThreadCleanupFn>,
    stack_start: Option<NonNull<u8>>,
    stack_size: usize,
    priority: u8,
    tick: u32,
    cpu: u8,
}

impl Default for ThreadBuilder {
    fn default() -> Self {
        Self {
            static_allocated: None,
            name: None,
            entry_fn: None,
            arg: core::ptr::null_mut(),
            cleanup_fn: None,
            stack_start: None,
            stack_size: 0,
            priority: 0,
            tick: 0,
            cpu: 0,
        }
    }
}

impl ThreadBuilder {
    pub fn new() -> Self {
        ThreadBuilder::default()
    }

    #[inline(always)]
    pub fn static_allocated(mut self, static_allocated: NonNull<Thread>) -> Self {
        self.static_allocated = Some(static_allocated);
        self
    }

    #[inline(always)]
    pub fn name(mut self, name: &'static ffi::CStr) -> Self {
        self.name = Some(name);
        self
    }

    #[inline(always)]
    pub fn entry_fn(mut self, entry_fn: ThreadEntryFn) -> Self {
        self.entry_fn = Some(entry_fn);
        self
    }

    #[inline(always)]
    pub fn arg(mut self, arg: *mut ffi::c_void) -> Self {
        self.arg = arg;
        self
    }

    #[inline(always)]
    pub fn cleanup_fn(mut self, cleanup_fn: ThreadCleanupFn) -> Self {
        self.cleanup_fn = Some(cleanup_fn);
        self
    }

    #[inline(always)]
    pub fn stack_start(mut self, stack_start: *mut u8) -> Self {
        assert!(
            !stack_start.is_null(),
            "User should have allocated valid stack space"
        );
        self.stack_start = NonNull::new(stack_start);
        self
    }

    #[inline(always)]
    pub fn stack_size(mut self, stack_size: usize) -> Self {
        assert!(stack_size > 0, "Stack size should not be zero");
        self.stack_size = stack_size;
        self
    }

    #[inline(always)]
    pub fn priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    #[inline(always)]
    pub fn tick(mut self, tick: u32) -> Self {
        assert!(
            tick > 0,
            "Tick should not be zero or the thread is unable to run"
        );
        self.tick = tick;
        self
    }

    // Must be noted, if a field implements non-trivial Drop, we must initialize it
    // via pinned-init. If the field is POD, we can use assignment to initialize it
    // or use core::ptr::write.
    fn raw_inplace_init(&mut self, ptr: *mut Thread) -> Result<(), &'static str> {
        let base_ptr = unsafe { (&mut ((*ptr).parent)) as *mut KObjectBase };
        let base = unsafe { &mut (*base_ptr) };
        if self.static_allocated.is_some() {
            base.init(
                ObjectClassType::ObjectClassThread,
                self.name.unwrap().as_ptr(),
            )
        } else {
            base.init_internal(
                ObjectClassType::ObjectClassThread,
                self.name.unwrap().as_ptr(),
            )
        }
        let thread = unsafe { &mut (*ptr) };
        let _ = unsafe { ListHead::new().__pinned_init(&mut thread.list_node as *mut ListHead) };
        thread.stat.set_base_state(ThreadState::INIT);
        thread.priority = ThreadPriority::new(self.priority);
        // If user doesn't specify an existing stack, we'll allocate one.
        let stack_start = self.stack_start.unwrap_or_else(|| {
            let stack_start = malloc(self.stack_size);
            thread.set_should_free_stack(true);
            unsafe { NonNull::new_unchecked(stack_start) }
        });
        thread.stack = Stack::new(stack_start.as_ptr(), self.stack_size);
        let sp = unsafe {
            Arch::init_task_stack(
                stack_start.as_ptr().offset(self.stack_size as isize) as *mut usize,
                stack_start.as_ptr() as *mut usize,
                mem::transmute(self.entry_fn.unwrap()),
                mem::transmute(self.arg),
                Thread::exit as *const usize,
            ) as *mut usize
        };
        thread.stack.set_sp(sp);
        thread.cleanup = self.cleanup_fn.take().unwrap_or(Thread::default_cleanup);
        let init = Timer::static_init(
            self.name.unwrap().as_ptr(),
            Thread::handle_timeout,
            thread as *mut _ as *mut ffi::c_void,
            0,
            (TimerState::ONE_SHOT.to_u32() | TimerState::THREAD_TIMER.to_u32()) as u8,
        );
        unsafe {
            let _ = init.__pinned_init(&mut thread.thread_timer as *mut Timer);
        }
        thread.tid = Thread::new_tid(ptr as usize);
        thread.spinlock = RawSpin::new();
        thread.error = code::EOK;

        #[cfg(schedule_with_time_slice)]
        {
            thread.time_slice = TimeSlice {
                init: self.tick,
                remaining: self.tick,
            };
        }

        #[cfg(smp)]
        {
            thread.cpu_affinity = CpuAffinity {
                bind_cpu: self.cpu,
                oncpu: CPUS_NUMBER as u8,
            };
        }

        #[cfg(mutex)]
        unsafe {
            let _ = MutexInfo::new().__pinned_init(&mut thread.mutex_info as *mut MutexInfo);
        }

        #[cfg(event)]
        {
            thread.event_info = EventInfo { set: 0, info: 0 };
        }

        #[cfg(debugging_spinlock)]
        {
            thread.lock_info = LockInfo {
                wait_lock: None,
                hold_locks: [None; 8],
                hold_count: 0,
            };
        }
        Ok(())
    }

    // PinInit is similar to C++ ctor's member initializer list, not ctor's body.
    pub fn build_pinned_init(mut self) -> impl PinInit<Thread> {
        let init = move |slot: *mut Thread| {
            let _ = self.raw_inplace_init(slot);
            Ok(())
        };
        unsafe { pin_init_from_closure(init) }
    }

    #[cfg(heap)]
    pub fn build_from_heap(self) -> Option<NonNull<Thread>> {
        let ptr = KObjectBase::new_raw(
            ObjectClassType::ObjectClassThread,
            (&self).name.as_ref().unwrap().as_ptr(),
        );
        let pinned_init = self.build_pinned_init();
        unsafe {
            let _ = pinned_init.__pinned_init(ptr as *mut Thread);
        }
        NonNull::new(ptr as *mut Thread)
    }

    pub fn build_from_static_allocation(self) -> Result<(), &'static str> {
        let Some(ptr) = self.static_allocated else {
            return Err("The thread object should be statically allocated");
        };
        let pinned_init = self.build_pinned_init();
        unsafe {
            let _ = pinned_init.__pinned_init(ptr.as_ptr());
        }
        Ok(())
    }
}
