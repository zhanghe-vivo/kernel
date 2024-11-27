#![allow(dead_code)]
use crate::alloc::boxed::Box;
use crate::{
    clock,
    cpu::{Cpu, CPUS_NUMBER},
    error::{code, Error},
    object::{self, KObjectBase, KernelObject, ObjectClassType},
    print, println,
    stack::Stack,
    static_init::UnsafeStaticInit,
    str::CStr,
    sync::{ipc_common::*, lock::mutex::*, RawSpin, SpinLock},
    timer::Timer,
    zombie,
};
#[cfg(feature = "RT_USING_SMP")]
use crate::cpu::CPU_DETACHED;
use alloc::alloc;
use blue_arch::arch::Arch;
use blue_arch::IScheduler;
use blue_infra::list::doubly_linked_list::ListHead;
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
use pinned_init::*;
use rt_bindings;

// compatible with C
pub type ThreadEntryFn = extern "C" fn(*mut ffi::c_void);
pub type ThreadCleanupFn = extern "C" fn(*mut RtThread);

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
        crate::container_of!($node, crate::thread::RtThread, tlist)
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
    id: SpinLock<[Cell<Option<NonNull<RtThread>>>; MAX_THREAD_SIZE]>,
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
    #[cfg(feature = "rt_thread_priority_max_256")]
    number: u8,
    #[cfg(feature = "rt_thread_priority_max_256")]
    high_mask: u8,
    number_mask: u32,
}

impl ThreadPriority {
    pub fn new(priority: u8) -> Self {
        #[cfg(feature = "rt_thread_priority_max_256")]
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
        #[cfg(not(feature = "rt_thread_priority_max_256"))]
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
        #[cfg(feature = "rt_thread_priority_max_256")]
        {
            self.number = new_priority >> 3;
            self.high_mask = 1 << (new_priority & 0x07);
            self.number_mask = 1 << self.number;
        }
        #[cfg(not(feature = "rt_thread_priority_max_256"))]
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

    #[cfg(feature = "rt_thread_priority_max_256")]
    #[inline]
    pub fn get_number(&self) -> u8 {
        self.number
    }

    #[cfg(feature = "rt_thread_priority_max_256")]
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

// pin_data conflict with cfg
#[cfg(feature = "RT_USING_MUTEX")]
#[repr(C)]
#[pin_data]
pub(crate) struct MutexInfo {
    #[pin]
    pub(crate) taken_list: ListHead,
    pub(crate) pending_to: Option<NonNull<RtMutex>>,
}

#[cfg(not(feature = "RT_USING_MUTEX"))]
#[repr(C)]
pub(crate) struct MutexInfo {}

#[cfg(feature = "RT_USING_MUTEX")]
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
    #[cfg(feature = "RT_USING_EVENT")]
    pub(crate) set: u32,
    #[cfg(feature = "RT_USING_EVENT")]
    pub(crate) info: u8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct CpuAffinity {
    #[cfg(feature = "RT_USING_SMP")]
    pub bind_cpu: u8,
    #[cfg(feature = "RT_USING_SMP")]
    pub oncpu: u8,
}

#[repr(C)]
#[derive(Debug)]
struct LockInfo {
    #[cfg(feature = "RT_DEBUGING_SPINLOCK")]
    pub wait_lock: Option<NonNull<RawSpin>>,
    #[cfg(feature = "RT_DEBUGING_SPINLOCK")]
    pub hold_locks: [Option<NonNull<RawSpin>>; 8],
    #[cfg(feature = "RT_DEBUGING_SPINLOCK")]
    pub hold_count: usize,
}

#[repr(C)]
#[pin_data(PinnedDrop)]
pub struct RtThread {
    #[pin]
    pub(crate) parent: KObjectBase,

    // thread linked list, link to priority_table
    #[pin]
    pub(crate) tlist: ListHead,

    /// thread status
    pub(crate) stat: ThreadState,

    /// priority manager
    pub(crate) priority: ThreadPriority,

    /// stack point and cleanup func
    pub(crate) stack: Stack,
    // can't add Option because of cbindgen
    cleanup: ThreadCleanupFn,
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

    #[cfg(feature = "RT_USING_MUTEX")]
    pub(crate) mutex_info: MutexInfo,

    #[cfg(feature = "RT_USING_EVENT")]
    pub(crate) event_info: EventInfo,

    /// cpu affinity
    #[cfg(feature = "RT_USING_SMP")]
    cpu_affinity: CpuAffinity,

    #[cfg(feature = "RT_DEBUGING_SPINLOCK")]
    lock_info: LockInfo,

    #[pin]
    pin: PhantomPinned,
}

// stack align to 8 bytes
#[repr(C, align(8))]
struct StackAlignedField<const STACK_SIZE: usize> {
    buf: [u8; STACK_SIZE],
}

#[pin_data]
pub struct ThreadWithStack<const STACK_SIZE: usize> {
    #[pin]
    pub(crate) inner: UnsafeCell<RtThread>,
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
            inner <- unsafe { pin_init_from_closure(move |slot: *mut UnsafeCell<RtThread>| {
                    let stack_addr = this.as_ref().stack.get_buf_ptr();
                    let init = RtThread::static_new(name, entry, parameter, stack_addr, STACK_SIZE, priority, tick);
                    init.__pinned_init(slot.cast::<RtThread>())
                }
            )},
        })
    }

    #[cfg(feature = "RT_USING_SMP")]
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
            inner <- unsafe { pin_init_from_closure(move |slot: *mut UnsafeCell<RtThread>| {
                    let stack_addr = this.as_ref().stack.get_buf_ptr();
                    let init = RtThread::new_with_bind(name, entry, parameter, stack_addr, STACK_SIZE, priority, tick, cpu);
                    init.__pinned_init(slot.cast::<RtThread>())
                }
            )},
        })
    }
}

impl<const STACK_SIZE: usize> core::ops::Deref for ThreadWithStack<STACK_SIZE> {
    type Target = RtThread;

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

impl RtThread {
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

    #[cfg(feature = "RT_USING_SMP")]
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
            let _ = unsafe { ListHead::new().__pinned_init(&mut cur_ref.tlist as *mut ListHead) };

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
                (rt_bindings::RT_TIMER_FLAG_ONE_SHOT | rt_bindings::RT_TIMER_FLAG_THREAD_TIMER)
                    as u8,
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

            #[cfg(feature = "RT_USING_SMP")]
            {
                cur_ref.cpu_affinity = CpuAffinity {
                    bind_cpu: _cpu,
                    oncpu: CPUS_NUMBER as u8,
                };
            }

            #[cfg(feature = "RT_USING_MUTEX")]
            unsafe {
                let _ = MutexInfo::new().__pinned_init(&mut cur_ref.mutex_info as *mut MutexInfo);
            }

            #[cfg(feature = "RT_USING_EVENT")]
            {
                cur_ref.event_info = EventInfo { set: 0, info: 0 };
            }

            #[cfg(feature = "RT_DEBUGING_SPINLOCK")]
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

    #[cfg(feature = "RT_USING_HEAP")]
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
        // need to alloc and drop stack manual
        let layout = unsafe {
            Layout::from_size_align_unchecked(stack_size, rt_bindings::RT_ALIGN_SIZE as usize)
        };
        let ptr = unsafe { alloc::alloc(layout) };

        match NonNull::new(ptr) {
            Some(_p) => {
                let thread = Box::pin_init(RtThread::dyn_new(
                    name, entry, parameter, ptr, stack_size, priority, tick,
                ));
                match thread {
                    Ok(_) => return thread,
                    Err(_) => {
                        // drop stack an return err
                        unsafe { alloc::dealloc(ptr, layout) };
                        return Err(AllocError);
                    }
                }
            }
            None => return Err(AllocError),
        }
    }

    // used for hw_context_switch
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

    #[cfg(feature = "RT_USING_SMP")]
    #[inline]
    pub fn is_cpu_detached(&self) -> bool {
        self.oncpu == CPU_DETACHED as u8
    }

    #[cfg(feature = "RT_USING_SMP")]
    #[inline]
    pub fn is_bind_cpu(&self) -> bool {
        self.bind_cpu != CPUS_NUMBER as u8
    }

    #[cfg(feature = "RT_USING_SMP")]
    #[inline]
    pub fn set_bind_cpu(&mut self, cpu_id: u8) {
        debug_assert!(cpu_id < CPUS_NUMBER as u8);
        self.bind_cpu = cpu_id;
    }

    #[cfg(feature = "RT_USING_SMP")]
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
                .load(Ordering::Relaxed),
        )
    }

    #[inline]
    pub(crate) fn get_name(&self) -> &CStr {
        unsafe { CStr::from_char_ptr(self.name()) }
    }

    #[inline]
    pub(crate) fn remove_tlist(&mut self) {
        unsafe { Pin::new_unchecked(&mut self.tlist).remove() };
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
    extern "C" fn default_cleanup(_thread: *mut RtThread) {}

    // thread timeout handler func.
    #[no_mangle]
    extern "C" fn handle_timeout(para: *mut ffi::c_void) {
        debug_assert!(para != ptr::null_mut());

        let thread = unsafe { &mut *(para as *mut RtThread) };
        debug_assert!(thread.parent.type_name() == ObjectClassType::ObjectClassThread as u8);

        let scheduler = Cpu::get_current_scheduler();
        let level = scheduler.sched_lock();

        debug_assert!(thread.stat.is_suspended());
        thread.error = code::ETIMEOUT;
        /* remove from suspend list */
        unsafe { Pin::new_unchecked(&mut thread.tlist).remove() };

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

    #[cfg(feature = "RT_USING_MUTEX")]
    #[inline]
    fn detach_from_mutex(&mut self) {
        let level = self.spinlock.lock_irqsave();

        // as rt_mutex_release may use sched_lock.
        if let Some(mut pending_mutex) = self.mutex_info.pending_to {
            unsafe {
                pending_mutex.as_mut().drop_thread(self);
            }
            self.mutex_info.pending_to = None;
        }

        let mut inspect = &self.mutex_info.taken_list;
        while let Some(next) = inspect.next() {
            inspect = unsafe { &*next.as_ptr() };
            if core::ptr::eq(inspect, &self.mutex_info.taken_list) {
                break;
            }
            unsafe {
                let mutex = crate::list_head_entry!(
                    inspect as *const ListHead as *mut ListHead,
                    RtMutex,
                    taken_list
                );
                if !mutex.is_null() {
                    (*mutex).release();
                }
            }
        }

        self.spinlock.unlock_irqrestore(level);
    }

    #[inline]
    pub(crate) fn get_mutex_priority(&self) -> u8 {
        let mut priority = self.priority.get_initial();

        crate::list_head_for_each!(node, &self.mutex_info.taken_list, {
            let mutex = unsafe { &*crate::list_head_entry!(node.as_ptr(), RtMutex, taken_list) };
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
    pub(crate) fn update_priority(&mut self, priority: u8, suspend_flag: u32) -> Error {
        let mut ret = code::EOK;
        // Change priority of the thread
        self.change_priority(priority);
        while self.stat.is_suspended() {
            //Whether change the priority of the taken mutex
            if let Some(mut pending_mutex) = self.mutex_info.pending_to {
                let pending_mutex = unsafe { pending_mutex.as_mut() };
                let owner_thread = unsafe { &mut *pending_mutex.owner };
                // Re-insert thread to suspended thread list
                self.remove_tlist();

                ret = Error::from_errno(IPCObject::suspend_thread(
                    &mut pending_mutex.parent.wait_list,
                    self,
                    pending_mutex.parent.flag as u8,
                    suspend_flag as u32,
                ));
                if ret == code::EOK {
                    // Update priority
                    pending_mutex.update_priority();
                    let mutex_priority = owner_thread.get_mutex_priority();
                    if mutex_priority != owner_thread.priority.get_current() {
                        owner_thread.change_priority(mutex_priority);
                    } else {
                        ret = code::ERROR;
                    }
                }
            }
        }
        ret
    }

    pub fn start(&mut self) {
        let scheduler = Cpu::get_current_scheduler();
        let level = scheduler.sched_lock();

        #[cfg(feature = "debug_scheduler")]
        println!("thread start: {:?}", self.get_name());

        self.priority.update(self.priority.get_current());
        // set to suspend.
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

        #[cfg(feature = "debug_scheduler")]
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
        // forbid scheduling on current core before returning since current thread
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
        #[cfg(feature = "debug_scheduler")]
        println!("thread detach: {:?}", self.get_name());

        self.close();

        #[cfg(feature = "RT_USING_MUTEX")]
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
        let tick = clock::rt_tick_from_millisecond(ms as i32);
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
        /* reset thread error */
        thread.error = code::EOK;

        #[cfg(feature = "debug_scheduler")]
        println!("thread sleep: {:?}", thread.get_name());

        if thread.suspend(SuspendFlag::Interruptible) {
            thread.thread_timer.restart(tick);
            thread.error = code::EINTR;
            // notify a pending rescheduling
            scheduler.do_task_schedule();
            // exit critical and do a rescheduling
            scheduler.preempt_enable();

            if thread.error == code::ETIMEOUT {
                thread.error = code::EOK;
            }
        }

        Ok(())
    }

    pub(crate) fn suspend(&mut self, suspend_flag: SuspendFlag) -> bool {
        assert!(self.is_current_runnung_thread());
        let scheduler = Cpu::get_current_scheduler();

        let level = scheduler.sched_lock();

        #[cfg(feature = "debug_scheduler")]
        println!("thread suspend: {:?}", self.get_name());

        if (!self.stat.is_ready()) && (!self.stat.is_running()) {
            println!("thread suspend: thread disorder, stat: {:?}", self.stat);
            scheduler.sched_unlock(level);
            return false;
        }

        // change thread stat
        scheduler.remove_thread_locked(self);
        #[cfg(feature = "RT_USING_SMP")]
        {
            self.oncpu = CPU_DETACHED as u8;
        }
        // set thread status as suspended
        self.stat.set_suspended(suspend_flag);
        // stop thread timer anyway
        self.timer_stop();
        scheduler.sched_unlock(level);

        return true;
    }

    pub fn resume(&mut self) -> bool {
        let scheduler = Cpu::get_current_scheduler();

        let level = scheduler.sched_lock();

        #[cfg(feature = "debug_scheduler")]
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
            // insert thread to schedule queue again
            scheduler.insert_thread_locked(self);
        } else {
            self.priority.update(priority);
        }
        scheduler.sched_unlock(level);
    }

    #[cfg(feature = "RT_USING_SMP")]
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
            // thread is running on a cpu
            let cur_cpu = scheduler.get_current_id();
            if cpu != CPUS_NUMBER as u8 {
                if cpu != cur_cpu {
                    unsafe {
                        rt_bindings::rt_hw_ipi_send(rt_bindings::RT_SCHEDULE_IPI as i32, 1 << cpu)
                    };
                    // self cpu need reschedule
                    scheduler.sched_unlock_with_sched(level);
                }
            } else {
                // no running on self cpu, but dest cpu can be itself
                unsafe {
                    rt_bindings::rt_hw_ipi_send(
                        rt_bindings::RT_SCHEDULE_IPI as i32,
                        1 << self.oncpu,
                    )
                };
                scheduler.sched_unlock(level);
            }
        }
    }

    #[cfg(feature = "RT_DEBUGING_SPINLOCK")]
    pub(crate) fn check_deadlock(&self, spin: &RawSpin) -> bool {
        let mut owner: Cell<Option<NonNull<RtThread>>> = spin.owner.clone();
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

    #[cfg(feature = "RT_DEBUGING_SPINLOCK")]
    pub(crate) fn set_wait(&mut self, spin: &RawSpin) {
        self.wait_lock = Some(NonNull::new(spin as *const _ as *mut _));
    }

    #[cfg(feature = "RT_DEBUGING_SPINLOCK")]
    pub(crate) fn clear_wait(&mut self) {
        self.lock_info.wait_lock = None;
    }
}

#[pinned_drop]
impl PinnedDrop for RtThread {
    fn drop(self: Pin<&mut Self>) {
        let this_th = unsafe { Pin::get_unchecked_mut(self) };

        #[cfg(feature = "debug_scheduler")]
        println!("drop thread: {:?}", this_th.get_name());

        this_th.detach();
    }
}

crate::impl_kobject!(RtThread);

#[no_mangle]
pub extern "C" fn rt_thread_init(
    thread: *mut RtThread,
    name: *const ffi::c_char,
    entry: ThreadEntryFn,
    parameter: *mut ffi::c_void,
    stack_start: *mut ffi::c_void,
    stack_size: rt_bindings::rt_uint32_t,
    priority: rt_bindings::rt_uint8_t,
    tick: rt_bindings::rt_uint32_t,
) -> rt_bindings::rt_err_t {
    // parameter check
    assert!(!thread.is_null());
    assert!(!stack_start.is_null());
    assert!(tick != 0);

    let name_cstr = unsafe { CStr::from_char_ptr(name) };
    let init = RtThread::static_new(
        name_cstr,
        entry,
        parameter as *mut usize,
        stack_start as *mut u8,
        stack_size as usize,
        priority,
        tick,
    );
    // no Error
    unsafe {
        let _ = init.__pinned_init(thread);
    }
    return rt_bindings::RT_EOK as rt_bindings::rt_err_t;
}

#[no_mangle]
pub extern "C" fn rt_thread_self() -> *mut RtThread {
    match Cpu::get_current_thread() {
        Some(thread) => thread.as_ptr(),
        None => ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn rt_thread_startup(thread: *mut RtThread) -> rt_bindings::rt_err_t {
    // parameter check
    assert!(!thread.is_null());
    assert!(
        object::rt_object_get_type(thread as rt_bindings::rt_object_t)
            == ObjectClassType::ObjectClassThread as u8
    );

    let th_mut = unsafe { &mut *thread };
    assert!(th_mut.stat.is_init());
    th_mut.start();

    return rt_bindings::RT_EOK as rt_bindings::rt_err_t;
}

#[no_mangle]
pub extern "C" fn rt_thread_close(thread: *mut RtThread) -> rt_bindings::rt_err_t {
    // parameter check
    assert!(!thread.is_null());
    assert!(
        object::rt_object_get_type(thread as rt_bindings::rt_object_t)
            == ObjectClassType::ObjectClassThread as u8
    );

    let th_mut = unsafe { &mut *thread };
    th_mut.close();

    return rt_bindings::RT_EOK as rt_bindings::rt_err_t;
}

#[no_mangle]
pub extern "C" fn rt_thread_detach(thread: *mut RtThread) -> rt_bindings::rt_err_t {
    // parameter check
    assert!(!thread.is_null());
    assert!(
        object::rt_object_get_type(thread as *mut rt_bindings::rt_object)
            == ObjectClassType::ObjectClassThread as u8
    );

    unsafe { (*thread).detach() };
    return rt_bindings::RT_EOK as rt_bindings::rt_err_t;
}

// #[cfg(feature = "RT_USING_HEAP")]
#[no_mangle]
pub extern "C" fn rt_thread_create(
    name: *const ffi::c_char,
    entry: ThreadEntryFn,
    parameter: *mut ffi::c_void,
    stack_size: rt_bindings::rt_uint32_t,
    priority: rt_bindings::rt_uint8_t,
    tick: rt_bindings::rt_uint32_t,
) -> *mut RtThread {
    let name_cstr = unsafe { CStr::from_char_ptr(name) };

    let thread = RtThread::try_new_in_heap(
        name_cstr,
        entry,
        parameter as *mut usize,
        stack_size as usize,
        priority,
        tick,
    );
    match thread {
        Ok(th) => {
            // need to free by zombie.
            unsafe { Box::leak(Pin::into_inner_unchecked(th)) }
        }
        Err(_) => return ptr::null_mut(),
    }
}

#[cfg(feature = "RT_USING_HEAP")]
#[no_mangle]
pub extern "C" fn rt_thread_delete(thread: *mut RtThread) -> rt_bindings::rt_err_t {
    assert!(!thread.is_null());
    assert!(
        object::rt_object_get_type(thread as rt_bindings::rt_object_t)
            == ObjectClassType::ObjectClassThread as u8
    );
    assert!(
        object::rt_object_is_systemobject(thread as rt_bindings::rt_object_t)
            == rt_bindings::RT_FALSE as i32
    );

    unsafe { (*thread).detach() };
    return rt_bindings::RT_EOK as rt_bindings::rt_err_t;
}

#[no_mangle]
pub extern "C" fn rt_thread_yield() -> rt_bindings::rt_err_t {
    RtThread::yield_now();
    return rt_bindings::RT_EOK as rt_bindings::rt_err_t;
}

#[no_mangle]
pub extern "C" fn rt_thread_delay(tick: rt_bindings::rt_tick_t) -> rt_bindings::rt_err_t {
    match RtThread::sleep(tick) {
        Ok(_) => return rt_bindings::RT_EOK as rt_bindings::rt_err_t,
        Err(e) => e.to_errno(),
    }
}

#[no_mangle]
pub extern "C" fn rt_thread_mdelay(ms: i32) -> rt_bindings::rt_err_t {
    let tick = clock::rt_tick_from_millisecond(ms);
    match RtThread::sleep(tick) {
        Ok(_) => return rt_bindings::RT_EOK as rt_bindings::rt_err_t,
        Err(e) => e.to_errno(),
    }
}

#[no_mangle]
pub extern "C" fn rt_thread_control(
    thread: *mut RtThread,
    cmd: u32,
    arg: *mut ffi::c_void,
) -> rt_bindings::rt_err_t {
    assert!(!thread.is_null());
    assert!(
        object::rt_object_get_type(thread as rt_bindings::rt_object_t)
            == ObjectClassType::ObjectClassThread as u8
    );
    let th = unsafe { &mut *thread };
    match cmd {
        rt_bindings::RT_THREAD_CTRL_CHANGE_PRIORITY => {
            let priority_ptr = NonNull::new(arg as *mut u8);
            if let Some(ptr) = priority_ptr {
                let priority = unsafe { *ptr.as_ref() };
                th.change_priority(priority);
            } else {
                return -(rt_bindings::RT_EINVAL as i32);
            }
        }
        rt_bindings::RT_THREAD_CTRL_STARTUP => {
            th.start();
        }
        rt_bindings::RT_THREAD_CTRL_CLOSE => {
            // detach will trigger schedule
            th.detach();
        }
        #[cfg(feature = "RT_USING_SMP")]
        rt_bindings::RT_THREAD_CTRL_BIND_CPU => {
            let cpu_ptr = NonNull::new(arg as *mut u8);
            if let Some(ptr) = cpu_ptr {
                let cpu = unsafe { *ptr.as_ref() };
                th.bind_to_cpu(cpu);
            } else {
                return -(rt_bindings::RT_EINVAL as i32);
            }
        }

        _ => {
            return -(rt_bindings::RT_EINVAL as i32);
        }
    }

    code::EOK.to_errno()
}

#[no_mangle]
pub extern "C" fn rt_thread_find(name: *mut ffi::c_char) -> *mut RtThread {
    return object::rt_object_find(name, ObjectClassType::ObjectClassThread as u8) as *mut RtThread;
}

#[no_mangle]
pub extern "C" fn rt_thread_get_name(
    thread: *mut RtThread,
    name: *mut ffi::c_char,
    name_size: u8,
) -> rt_bindings::rt_err_t {
    return if thread.is_null() {
        -(rt_bindings::RT_EINVAL as i32)
    } else {
        object::rt_object_get_name(thread as *mut rt_bindings::rt_object, name, name_size)
    };
}

#[no_mangle]
pub extern "C" fn rt_thread_suspend_with_flag(
    thread: *mut RtThread,
    suspend_flag: u32,
) -> rt_bindings::rt_err_t {
    assert!(!thread.is_null());
    assert!(
        object::rt_object_get_type(thread as rt_bindings::rt_object_t)
            == ObjectClassType::ObjectClassThread as u8
    );

    let th = unsafe { &mut *thread };
    if th.suspend(SuspendFlag::from_u8(suspend_flag as u8)) {
        return rt_bindings::RT_EOK as rt_bindings::rt_err_t;
    }
    -(rt_bindings::RT_ERROR as rt_bindings::rt_err_t)
}

#[no_mangle]
pub extern "C" fn rt_thread_suspend(thread: *mut RtThread) -> rt_bindings::rt_err_t {
    rt_thread_suspend_with_flag(thread, rt_bindings::RT_UNINTERRUPTIBLE)
}

#[no_mangle]
pub extern "C" fn rt_thread_resume(thread: *mut RtThread) -> rt_bindings::rt_err_t {
    assert!(!thread.is_null());
    assert!(
        object::rt_object_get_type(thread as rt_bindings::rt_object_t)
            == ObjectClassType::ObjectClassThread as u8
    );

    let th = unsafe { &mut *thread };
    if th.resume() {
        rt_bindings::RT_EOK as rt_bindings::rt_err_t
    } else {
        -(rt_bindings::RT_ERROR as rt_bindings::rt_err_t)
    }
}

#[no_mangle]
pub extern "C" fn rt_thread_cleanup(thread: *mut RtThread, cleanup: ThreadCleanupFn) {
    assert!(!thread.is_null());
    assert!(cleanup as *const () != core::ptr::null());

    unsafe { (*thread).cleanup = cleanup };
}

#[no_mangle]
pub extern "C" fn rt_thread_info() {
    let callback_forword = || {
        #[cfg(feature = "RT_USING_SMP")]
        crate::kprintf!(
            "thread   cpu bind pri  status      sp     stack size left tick  error\n\0"
        );
        #[cfg(not(feature = "RT_USING_SMP"))]
        crate::kprintf!("thread   pri  status      sp     stack size left tick  error\n\0");
        #[cfg(feature = "RT_USING_SMP")]
        crate::kprintf!("-------- --- ---- ---  ------- ---------- ----------  ---------- ---\n\0");
        #[cfg(not(feature = "RT_USING_SMP"))]
        crate::kprintf!("-------- ---  ------- ---------- ----------  ---------- ---\n\0");
    };
    let callback = |node: &ListHead| unsafe {
        let thread =
            &*(crate::list_head_entry!(node.as_ptr(), KObjectBase, list) as *const RtThread);
        let _ = crate::format_name!(thread.parent.name.as_ptr(), 8);
        let current_priority = thread.priority.get_current();
        #[cfg(feature = "RT_USING_SMP")]
        {
            let oncpu = thread.oncpu;
            if oncpu != CPU_DETACHED as u8 {
                print!(
                    " {:<3} {:^4}  {:<3} ",
                    oncpu, thread.bind_cpu, current_priority
                );
            } else {
                print!(" N/A {:^4}  {:<3} ", thread.bind_cpu, current_priority);
            }
        }
        #[cfg(not(feature = "RT_USING_SMP"))]
        {
            print!(" {:<3}  ", current_priority);
        }
        let state = thread.stat;
        if state.is_ready() {
            print!("ready  ")
        } else if state.is_suspended() {
            print!("suspend")
        } else if state.is_init() {
            print!("init   ")
        } else if state.is_close() {
            print!("close  ")
        } else if state.is_running() {
            print!("running")
        }
        println!(
            " 0x{:08x} 0x{:08x}  0x{:08x} {}",
            thread
                .stack
                .bottom_ptr()
                .sub(thread.stack.sp() as usize)
                .add(thread.stack.size()) as usize,
            thread.stack.size(),
            thread.time_slice.remaining,
            thread.error.to_errno()
        );
    };
    let _ = RtThread::get_info(
        callback_forword,
        callback,
        ObjectClassType::ObjectClassThread as u8,
    );
}
