#![allow(dead_code)]
#[cfg(feature = "smp")]
use crate::cpu::CPU_DETACHED;
use crate::{
    alloc::boxed::Box,
    blue_kconfig::ALIGN_SIZE,
    clock,
    cpu::{Cpu, CPUS_NUMBER},
    error::{code, Error},
    object::{KObjectBase, KernelObject, ObjectClassType},
    println,
    stack::Stack,
    static_init::UnsafeStaticInit,
    str::CStr,
    sync::{lock::mutex::*, RawSpin, SpinLock},
    timer::{Timer, TimerState},
    zombie,
};
use alloc::alloc;
use blue_arch::arch::Arch;
use blue_infra::list::doubly_linked_list::{LinkedListNode, ListHead};
use core::{
    alloc::{AllocError, Layout},
    cell::{Cell, UnsafeCell},
    ffi,
    marker::PhantomPinned,
    mem,
    pin::Pin,
    ptr::{self, NonNull},
    sync::atomic::{AtomicUsize, Ordering},
};
use pinned_init::{
    init, pin_data, pin_init, pin_init_array_from_fn, pin_init_from_closure, pinned_drop, zeroed,
    InPlaceInit, Init, PinInit,
};

// compatible with C
pub type ThreadEntryFn = extern "C" fn(*mut ffi::c_void);
pub type ThreadCleanupFn = extern "C" fn(*mut Thread);

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

#[macro_export]
macro_rules! thread_list_node_entry {
    ($node:expr) => {
        crate::container_of!($node, crate::thread::Thread, list_node)
    };
}
pub use thread_list_node_entry;

const MAX_THREAD_SIZE: usize = 1024;
pub(crate) static mut TIDS: UnsafeStaticInit<Tid, TidInit> = UnsafeStaticInit::new(TidInit);

pub(crate) struct TidInit;
unsafe impl PinInit<Tid> for TidInit {
    unsafe fn __pinned_init(self, slot: *mut Tid) -> Result<(), core::convert::Infallible> {
        let init = Tid::new();
        unsafe { init.__pinned_init(slot) }
    }
}

#[pin_data]
pub(crate) struct Tid {
    #[pin]
    id: SpinLock<[Cell<Option<NonNull<Thread>>>; MAX_THREAD_SIZE]>,
}

impl Tid {
    fn new() -> impl PinInit<Self> {
        pin_init!(Self {
            id <- crate::new_spinlock!(pin_init_array_from_fn(|_| Cell::new(None))),
        })
    }
}

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
    #[cfg(feature = "thread_priority_max")]
    number: u8,
    #[cfg(feature = "thread_priority_max")]
    high_mask: u8,
    number_mask: u32,
}

impl ThreadPriority {
    pub fn new(priority: u8) -> Self {
        #[cfg(feature = "thread_priority_max")]
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
        #[cfg(not(feature = "thread_priority_max"))]
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
        #[cfg(feature = "thread_priority_max")]
        {
            self.number = new_priority >> 3;
            self.high_mask = 1 << (new_priority & 0x07);
            self.number_mask = 1 << self.number;
        }
        #[cfg(not(feature = "thread_priority_max"))]
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

    #[cfg(feature = "thread_priority_max")]
    #[inline]
    pub fn get_number(&self) -> u8 {
        self.number
    }

    #[cfg(feature = "thread_priority_max")]
    #[inline]
    pub fn get_high_mask(&self) -> u8 {
        self.high_mask
    }

    /// 获取组掩码
    #[inline]
    pub fn get_number_mask(&self) -> u32 {
        self.number_mask
    }
}

#[cfg(feature = "schedule_with_time_slice")]
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct TimeSlice {
    pub init: u32,
    pub remaining: u32,
}

// NOTE: pin_data conflicts with cfg.
#[cfg(feature = "mutex")]
#[repr(C)]
#[pin_data]
pub(crate) struct MutexInfo {
    #[pin]
    pub(crate) taken_list: ListHead,
    pub(crate) pending_to: Option<NonNull<Mutex>>,
}

#[cfg(not(feature = "mutex"))]
#[repr(C)]
pub(crate) struct MutexInfo {}

#[cfg(feature = "mutex")]
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
    #[cfg(feature = "event")]
    pub(crate) set: u32,
    #[cfg(feature = "event")]
    pub(crate) info: u8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct CpuAffinity {
    #[cfg(feature = "smp")]
    pub bind_cpu: u8,
    #[cfg(feature = "smp")]
    pub oncpu: u8,
}

#[repr(C)]
#[derive(Debug)]
struct LockInfo {
    #[cfg(feature = "debugging_spinlock")]
    pub wait_lock: Option<NonNull<RawSpin>>,
    #[cfg(feature = "debugging_spinlock")]
    pub hold_locks: [Option<NonNull<RawSpin>>; 8],
    #[cfg(feature = "debugging_spinlock")]
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
    #[cfg(feature = "schedule_with_time_slice")]
    time_slice: TimeSlice,

    #[cfg(feature = "mutex")]
    pub(crate) mutex_info: MutexInfo,

    #[cfg(feature = "event")]
    pub(crate) event_info: EventInfo,

    /// cpu affinity
    #[cfg(feature = "smp")]
    cpu_affinity: CpuAffinity,

    #[cfg(feature = "debugging_spinlock")]
    lock_info: LockInfo,

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
        name: &'static CStr,
        entry: ThreadEntryFn,
        parameter: *mut usize,
        priority: u8,
        tick: u32,
    ) -> impl PinInit<Self> {
        pin_init!(&this in Self {
            stack <- StackAlignedField::<STACK_SIZE>::new(),
            inner <- unsafe { pin_init_from_closure(move |slot: *mut UnsafeCell<Thread>| {
                    let stack_addr = this.as_ref().stack.get_buf_ptr();
                    let init = Thread::static_new(name, entry, parameter, stack_addr, STACK_SIZE, priority, tick);
                    init.__pinned_init(slot.cast::<Thread>())
                }
            )},
        })
    }

    #[cfg(feature = "smp")]
    #[inline]
    pub fn new_with_bind(
        name: &'static CStr,
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
                    let init = Thread::new_with_bind(name, entry, parameter, stack_addr, STACK_SIZE, priority, tick, cpu);
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
    #[inline]
    pub fn static_new(
        name: &'static CStr,
        entry: ThreadEntryFn,
        parameter: *mut usize,
        stack_start: *mut u8,
        stack_size: usize,
        priority: u8,
        tick: u32,
    ) -> impl PinInit<Self> {
        Self::new_internal(
            name,
            entry,
            parameter,
            stack_start,
            stack_size,
            priority,
            tick,
            CPUS_NUMBER as u8,
            true,
        )
    }

    #[inline]
    pub fn dyn_new(
        name: &'static CStr,
        entry: ThreadEntryFn,
        parameter: *mut usize,
        stack_start: *mut u8,
        stack_size: usize,
        priority: u8,
        tick: u32,
    ) -> impl PinInit<Self> {
        Self::new_internal(
            name,
            entry,
            parameter,
            stack_start,
            stack_size,
            priority,
            tick,
            CPUS_NUMBER as u8,
            false,
        )
    }

    #[cfg(feature = "smp")]
    #[inline]
    pub(crate) fn new_with_bind(
        name: &'static CStr,
        entry: ThreadEntryFn,
        parameter: *mut usize,
        stack_start: *mut u8,
        stack_size: usize,
        priority: u8,
        tick: u32,
        cpu: u8,
    ) -> impl PinInit<Self> {
        Self::new_internal(
            name,
            entry,
            parameter,
            stack_start,
            stack_size,
            priority,
            tick,
            cpu,
            true,
        )
    }

    fn new_internal(
        name: &'static CStr,
        entry: ThreadEntryFn,
        parameter: *mut usize,
        stack_start: *mut u8,
        stack_size: usize,
        priority: u8,
        tick: u32,
        _cpu: u8,
        is_static: bool,
    ) -> impl PinInit<Self> {
        let init = move |slot: *mut Self| {
            let obj = unsafe { &mut *(slot as *mut KObjectBase) };
            if is_static {
                obj.init(ObjectClassType::ObjectClassThread as u8, name.as_char_ptr())
            } else {
                obj.init_internal(ObjectClassType::ObjectClassThread as u8, name.as_char_ptr())
            }

            let cur_ref = unsafe { &mut *(slot as *mut Self) };
            let _ = unsafe {
                LinkedListNode::new().__pinned_init(&mut cur_ref.list_node as *mut LinkedListNode)
            };

            cur_ref.stat.set_base_state(ThreadState::INIT);
            cur_ref.priority = ThreadPriority::new(priority);

            cur_ref.stack = Stack::new(stack_start, stack_size);
            let sp = unsafe {
                Arch::init_task_stack(
                    stack_start.offset(stack_size as isize) as *mut usize,
                    stack_start as *mut usize,
                    mem::transmute(entry),
                    parameter,
                    Self::exit as *const usize,
                ) as *mut usize
            };
            cur_ref.stack.set_sp(sp);
            cur_ref.cleanup = Self::default_cleanup;

            let init = Timer::static_init(
                name.as_char_ptr(),
                Self::handle_timeout,
                cur_ref as *mut _ as *mut ffi::c_void,
                0,
                (TimerState::ONE_SHOT.to_u32() | TimerState::THREAD_TIMER.to_u32()) as u8,
            );
            unsafe {
                let _ = init.__pinned_init(&mut cur_ref.thread_timer as *mut Timer);
            }

            cur_ref.tid = Self::new_tid();
            unsafe {
                (&raw const TIDS as *const UnsafeStaticInit<Tid, TidInit>)
                    .as_ref()
                    .unwrap_unchecked()
                    .id
                    .lock()[cur_ref.tid as usize]
                    .set(Some(NonNull::new_unchecked(slot)));
            }

            cur_ref.spinlock = RawSpin::new();
            cur_ref.error = code::EOK;

            #[cfg(feature = "schedule_with_time_slice")]
            {
                cur_ref.time_slice = TimeSlice {
                    init: tick,
                    remaining: tick,
                };
            }

            #[cfg(feature = "smp")]
            {
                cur_ref.cpu_affinity = CpuAffinity {
                    bind_cpu: _cpu,
                    oncpu: CPUS_NUMBER as u8,
                };
            }

            #[cfg(feature = "mutex")]
            unsafe {
                let _ = MutexInfo::new().__pinned_init(&mut cur_ref.mutex_info as *mut MutexInfo);
            }

            #[cfg(feature = "event")]
            {
                cur_ref.event_info = EventInfo { set: 0, info: 0 };
            }

            #[cfg(feature = "debugging_spinlock")]
            {
                cur_ref.lock_info = LockInfo {
                    wait_lock: None,
                    hold_locks: [None; 8],
                    hold_count: 0,
                };
            }
            Ok(())
        };
        unsafe { pin_init_from_closure(init) }
    }

    #[cfg(feature = "heap")]
    pub fn try_new_in_heap(
        name: &'static CStr,
        entry: ThreadEntryFn,
        parameter: *mut usize,
        stack_size: usize,
        priority: u8,
        tick: u32,
    ) -> Result<Pin<Box<Self>>, AllocError> {
        assert!(tick != 0);
        assert!(stack_size != 0);
        // Need to alloc and drop stack manually.
        let layout = unsafe { Layout::from_size_align_unchecked(stack_size, ALIGN_SIZE as usize) };
        let ptr = unsafe { alloc::alloc(layout) };

        match NonNull::new(ptr) {
            Some(_p) => {
                let thread = Box::pin_init(Thread::dyn_new(
                    name, entry, parameter, ptr, stack_size, priority, tick,
                ));
                match thread {
                    Ok(_) => return thread,
                    Err(_) => {
                        unsafe { alloc::dealloc(ptr, layout) };
                        return Err(AllocError);
                    }
                }
            }
            None => return Err(AllocError),
        }
    }

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

        #[cfg(feature = "schedule_with_time_slice")]
        {
            self.time_slice.remaining = self.time_slice.init;
        }
        self.stat.add_yield();
    }

    #[cfg(feature = "smp")]
    #[inline]
    pub fn is_cpu_detached(&self) -> bool {
        self.oncpu == CPU_DETACHED as u8
    }

    #[cfg(feature = "smp")]
    #[inline]
    pub fn is_bind_cpu(&self) -> bool {
        self.bind_cpu != CPUS_NUMBER as u8
    }

    #[cfg(feature = "smp")]
    #[inline]
    pub fn set_bind_cpu(&mut self, cpu_id: u8) {
        debug_assert!(cpu_id < CPUS_NUMBER as u8);
        self.bind_cpu = cpu_id;
    }

    #[cfg(feature = "smp")]
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
    pub(crate) fn get_name(&self) -> &CStr {
        unsafe { CStr::from_char_ptr(self.name()) }
    }

    #[inline]
    pub(crate) fn remove_thread_list_node(&mut self) {
        unsafe { Pin::new_unchecked(&mut self.list_node).remove_from_list() };
    }

    #[inline]
    pub(crate) fn new_tid() -> usize {
        static TID: AtomicUsize = AtomicUsize::new(0);
        let id = TID.fetch_add(1, Ordering::SeqCst);
        let tids = unsafe {
            (&raw const TIDS as *const UnsafeStaticInit<Tid, TidInit>)
                .as_ref()
                .unwrap_unchecked()
                .id
                .lock()
        };
        if id >= MAX_THREAD_SIZE || !tids[id].get().is_none() {
            for i in 0..MAX_THREAD_SIZE {
                if tids[i].get().is_none() {
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
        debug_assert!(thread.type_name() == ObjectClassType::ObjectClassThread as u8);

        let scheduler = Cpu::get_current_scheduler();
        let level = scheduler.sched_lock();

        debug_assert!(thread.stat.is_suspended());
        thread.error = code::ETIMEDOUT;
        thread.remove_thread_list_node();
        scheduler.insert_thread_locked(thread);

        scheduler.sched_unlock_with_sched(level);
    }

    unsafe extern "C" fn exit() {
        let th = crate::current_thread!().unwrap().as_mut();
        th.detach();
        panic!("!!! never get here !!!, thread {}", th.get_name());
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

        #[cfg(feature = "debugging_scheduler")]
        println!("thread start: {:?}", self.get_name());

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

        #[cfg(feature = "debugging_scheduler")]
        println!("thread close: {:?}", self.get_name());

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
        unsafe {
            (&raw const TIDS as *const UnsafeStaticInit<Tid, TidInit>)
                .as_ref()
                .unwrap_unchecked()
                .id
                .lock()[self.tid as usize]
                .set(None);
        }
        #[cfg(feature = "debugging_scheduler")]
        println!("thread detach: {:?}", self.get_name());

        self.close();

        #[cfg(feature = "mutex")]
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

        #[cfg(feature = "debugging_scheduler")]
        println!("thread sleep: {:?}", thread.get_name());

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

        #[cfg(feature = "debugging_scheduler")]
        println!("thread suspend: {:?}", self.get_name());

        if (!self.stat.is_ready()) && (!self.stat.is_running()) {
            println!("thread suspend: thread disorder, stat: {:?}", self.stat);
            scheduler.sched_unlock(level);
            return false;
        }

        scheduler.remove_thread_locked(self);
        #[cfg(feature = "smp")]
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

        #[cfg(feature = "debugging_scheduler")]
        println!("thread resume: {:?}", self.get_name());

        let need_schedule = scheduler.insert_ready_locked(self);
        // if need_schedule {
        //     scheduler.sched_unlock_with_sched(level);
        // } else {
        scheduler.sched_unlock(level);
        // }

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

    #[cfg(feature = "smp")]
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

    #[cfg(feature = "debugging_spinlock")]
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

    #[cfg(feature = "debugging_spinlock")]
    pub(crate) fn set_wait(&mut self, spin: &RawSpin) {
        self.wait_lock = Some(NonNull::new(spin as *const _ as *mut _));
    }

    #[cfg(feature = "debugging_spinlock")]
    pub(crate) fn clear_wait(&mut self) {
        self.lock_info.wait_lock = None;
    }
}

#[pinned_drop]
impl PinnedDrop for Thread {
    fn drop(self: Pin<&mut Self>) {
        let this_th = unsafe { Pin::get_unchecked_mut(self) };

        #[cfg(feature = "debugging_scheduler")]
        println!("drop thread: {:?}", this_th.get_name());

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
