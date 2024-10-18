#![allow(dead_code)]
use crate::alloc::boxed::Box;
use crate::{
    clock,
    cpu::Cpu,
    error::{code, Error},
    linked_list::ListHead,
    object,
    object::{BaseObject, ObjectClassType},
    println, rt_bindings,
    stack::Stack,
    str::CStr,
    sync::RawSpin,
    timer, zombie,
};
use alloc::alloc;
use core::{
    alloc::{AllocError, Layout},
    cell::{Cell, UnsafeCell},
    ffi,
    marker::PhantomPinned,
    mem,
    pin::Pin,
    ptr::{self, NonNull},
    sync::atomic::Ordering,
};
use pinned_init::*;

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
macro_rules! thread_list_node_entry {
    ($node:expr) => {
        crate::container_of!($node, crate::thread::RtThread, tlist)
    };
}
pub use thread_list_node_entry;

#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
type RtThreadHook = extern "C" fn(thread: *mut RtThread);
#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
static mut RT_THREAD_SUSPEND_HOOK: Option<RtThreadHook> = None;
#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
static mut RT_THREAD_RESUME_HOOK: Option<RtThreadHook> = None;

#[repr(C)]
// #[derive(Debug)]
#[pin_data(PinnedDrop)]
pub struct RtThread {
    #[pin]
    pub(crate) parent: BaseObject,
    // start of schedule context
    /// the thread list, used in ready_list\ipc wait_list\...
    #[pin]
    pub(crate) tlist: ListHead,
    /// thread status
    pub(crate) stat: ffi::c_uchar,
    pub(crate) sched_flag_ttmr_set: ffi::c_uchar,
    /// priority manager
    pub(crate) current_priority: ffi::c_uchar,
    init_priority: ffi::c_uchar,
    // FIXME #[cfg(RT_THREAD_PRIORITY_MAX > 32)]
    pub(crate) number: ffi::c_uchar,
    // FIXME #[cfg(RT_THREAD_PRIORITY_MAX > 32)]
    pub(crate) high_mask: ffi::c_uchar,
    pub(crate) number_mask: ffi::c_uint,
    /// priority number mask

    /// time slice
    init_tick: ffi::c_uint,
    remaining_tick: ffi::c_uint,

    /// built-in thread timer, used for wait timeout
    #[pin]
    pub thread_timer: rt_bindings::rt_timer,

    /// stack point and entry
    pub(crate) stack: Stack,
    entry: *mut ffi::c_void,
    parameter: *mut ffi::c_void,
    cleanup: *mut ffi::c_void,

    /// thread binds to cpu
    #[cfg(feature = "RT_USING_SMP")]
    bind_cpu: ffi::c_uchar,
    /// running on cpu id
    #[cfg(feature = "RT_USING_SMP")]
    pub(crate) oncpu: ffi::c_uchar,
    /// critical lock count
    // #[cfg(feature = "RT_USING_SMP")]
    // critical_lock_nest: ffi::c_uint,
    // /// cpus lock count
    // #[cfg(feature = "RT_USING_SMP")]
    // cpus_lock_nest: AtomicU32,
    spinlock: RawSpin,
    /// error code
    error: ffi::c_int,

    /// mutexes holded by this thread
    #[cfg(feature = "RT_USING_MUTEX")]
    #[pin]
    taken_object_list: ListHead,
    /// mutex object
    #[cfg(feature = "RT_USING_MUTEX")]
    pending_object: *mut rt_bindings::rt_object,

    #[cfg(feature = "RT_USING_EVENT")]
    event_set: ffi::c_uint,
    #[cfg(feature = "RT_USING_EVENT")]
    event_info: ffi::c_uchar,

    #[cfg(feature = "RT_DEBUGING_SPINLOCK")]
    wait_lock: Cell<Option<NonNull<RawSpin>>>,
    #[cfg(feature = "RT_DEBUGING_SPINLOCK")]
    hold_locks: [Cell<Option<NonNull<RawSpin>>>; 8],
    #[cfg(feature = "RT_DEBUGING_SPINLOCK")]
    hold_count: usize,

    // signal pthread not used yet.
    #[pin]
    pin: PhantomPinned,
}

// FIXME. use RT_ALIGN_SIZE
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
        parameter: *mut ffi::c_void,
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
        parameter: *mut ffi::c_void,
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
        parameter: *mut ffi::c_void,
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
            rt_bindings::RT_CPUS_NR as u8,
            true,
        )
    }

    #[inline]
    pub fn dyn_new(
        name: &'static CStr,
        entry: ThreadEntryFn,
        parameter: *mut ffi::c_void,
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
            rt_bindings::RT_CPUS_NR as u8,
            false,
        )
    }

    #[cfg(feature = "RT_USING_SMP")]
    #[inline]
    pub(crate) fn new_with_bind(
        name: &'static CStr,
        entry: ThreadEntryFn,
        parameter: *mut ffi::c_void,
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
        parameter: *mut ffi::c_void,
        stack_start: *mut u8,
        stack_size: usize,
        priority: u8,
        tick: u32,
        cpu: u8,
        is_static: bool,
    ) -> impl PinInit<Self> {
        let init = move |slot: *mut Self| unsafe {
            if is_static {
                object::rt_object_init(
                    slot as *mut rt_bindings::rt_object,
                    ObjectClassType::ObjectClassThread as u32,
                    name.as_char_ptr(),
                )
            } else {
                object::rt_object_init_dyn(
                    slot as *mut rt_bindings::rt_object,
                    ObjectClassType::ObjectClassThread as u32,
                    name.as_char_ptr(),
                )
            }

            let cur_ref = &mut *slot;
            let _ = ListHead::new().__pinned_init(&mut cur_ref.tlist as *mut ListHead);

            timer::rt_timer_init(
                &mut cur_ref.thread_timer as *mut _ as *mut timer::Timer,
                name.as_char_ptr(),
                Self::handle_timeout,
                cur_ref as *mut _ as *mut ffi::c_void,
                0,
                (rt_bindings::RT_TIMER_FLAG_ONE_SHOT | rt_bindings::RT_TIMER_FLAG_THREAD_TIMER)
                    as u8,
            );

            cur_ref.stat = rt_bindings::RT_THREAD_INIT as u8;
            cur_ref.sched_flag_ttmr_set = 0;
            cur_ref.current_priority = priority;
            cur_ref.init_priority = priority;
            // FIXME #[cfg(RT_THREAD_PRIORITY_MAX > 32)]
            cur_ref.number = 0;
            // FIXME #[cfg(RT_THREAD_PRIORITY_MAX > 32)]
            cur_ref.high_mask = 0;
            cur_ref.number_mask = 0;
            cur_ref.init_tick = tick;
            cur_ref.remaining_tick = tick;

            #[cfg(feature = "RT_USING_SMP")]
            {
                cur_ref.bind_cpu = cpu;
                cur_ref.oncpu = rt_bindings::RT_CPU_DETACHED as u8;
                // cur_ref.critical_lock_nest = 0;
                // cur_ref.cpus_lock_nest = AtomicU32::new(0);
            }

            let sp = rt_bindings::rt_hw_stack_init(
                mem::transmute(entry),
                parameter,
                stack_start
                    .offset((stack_size - mem::size_of::<rt_bindings::rt_ubase_t>()) as isize),
                Self::exit as *mut ffi::c_void,
            ) as *mut usize;
            cur_ref.stack = Stack::new(stack_start, stack_size);
            cur_ref.stack.set_sp(sp);
            cur_ref.entry = mem::transmute(entry);
            cur_ref.parameter = parameter;
            cur_ref.cleanup = ptr::null_mut();

            #[cfg(feature = "RT_USING_MUTEX")]
            {
                // no Error
                let _ =
                    ListHead::new().__pinned_init(&mut cur_ref.taken_object_list as *mut ListHead);
                cur_ref.pending_object = ptr::null_mut();
            }

            #[cfg(feature = "RT_USING_EVENT")]
            {
                cur_ref.event_set = 0;
                cur_ref.event_info = 0;
            }
            cur_ref.spinlock = RawSpin::new();
            cur_ref.error = rt_bindings::RT_EOK as i32;

            #[cfg(feature = "RT_DEBUGING_SPINLOCK")]
            {
                const ARRAY_REPEAT_VALUE: Cell<Option<NonNull<RawSpin>>> = Cell::new(None);
                cur_ref.wait_lock = ARRAY_REPEAT_VALUE;
                cur_ref.hold_locks = [ARRAY_REPEAT_VALUE; 8];
                cur_ref.hold_count = 0;
            }
            Ok(())
        };
        unsafe { pin_init_from_closure(init) }
    }

    #[cfg(feature = "RT_USING_HEAP")]
    pub fn try_new_in_heap(
        name: &'static CStr,
        entry: ThreadEntryFn,
        parameter: *mut ffi::c_void,
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

    #[inline]
    pub(crate) fn get_stat(&self) -> u8 {
        self.stat & (rt_bindings::RT_THREAD_STAT_MASK as u8)
    }

    #[inline]
    pub(crate) fn is_init_stat(&self) -> bool {
        (self.stat & (rt_bindings::RT_THREAD_STAT_MASK as u8))
            == (rt_bindings::RT_THREAD_INIT as u8)
    }

    #[inline]
    pub(crate) fn is_ready(&self) -> bool {
        debug_assert!(Cpu::get_current_scheduler().is_sched_locked());
        (self.stat & (rt_bindings::RT_THREAD_STAT_MASK as u8))
            == (rt_bindings::RT_THREAD_READY as u8)
    }

    #[inline]
    pub fn set_ready(&mut self) {
        debug_assert!(Cpu::get_current_scheduler().is_sched_locked());
        self.stat = rt_bindings::RT_THREAD_READY as u8
            | (self.stat & !(rt_bindings::RT_THREAD_STAT_MASK as u8));
    }

    #[inline]
    pub fn is_suspended(&self) -> bool {
        debug_assert!(Cpu::get_current_scheduler().is_sched_locked());
        (self.stat & (rt_bindings::RT_THREAD_SUSPEND_MASK as u8))
            == (rt_bindings::RT_THREAD_SUSPEND as u8)
    }

    #[inline]
    pub fn set_suspended(&mut self) {
        debug_assert!(Cpu::get_current_scheduler().is_sched_locked());
        self.stat = rt_bindings::RT_THREAD_SUSPEND as u8
    }

    #[inline]
    pub fn is_yield(&self) -> bool {
        debug_assert!(Cpu::get_current_scheduler().is_sched_locked());
        (self.stat & rt_bindings::RT_THREAD_STAT_YIELD_MASK as u8) != 0
    }

    #[inline]
    pub fn add_yield(&mut self) {
        debug_assert!(Cpu::get_current_scheduler().is_sched_locked());
        self.stat |= rt_bindings::RT_THREAD_STAT_YIELD as u8;
    }

    #[inline]
    pub fn reset_to_yield(&mut self) {
        debug_assert!(Cpu::get_current_scheduler().is_sched_locked());
        self.remaining_tick = self.init_tick;
        self.stat |= rt_bindings::RT_THREAD_STAT_YIELD as u8;
    }

    #[inline]
    pub fn is_running(&self) -> bool {
        debug_assert!(Cpu::get_current_scheduler().is_sched_locked());
        (self.stat & (rt_bindings::RT_THREAD_STAT_MASK as u8))
            == (rt_bindings::RT_THREAD_RUNNING as u8)
    }

    #[inline]
    pub fn set_running(&mut self) {
        debug_assert!(Cpu::get_current_scheduler().is_sched_locked());
        self.stat = rt_bindings::RT_THREAD_RUNNING as u8
            | (self.stat & !(rt_bindings::RT_THREAD_STAT_MASK as u8));
    }

    #[inline]
    pub fn set_init_stat(&mut self) {
        self.stat = rt_bindings::RT_THREAD_INIT as u8;
    }

    #[cfg(feature = "RT_USING_SMP")]
    #[inline]
    pub fn is_cpu_detached(&self) -> bool {
        self.oncpu == rt_bindings::RT_CPU_DETACHED as u8
    }

    #[cfg(feature = "RT_USING_SMP")]
    #[inline]
    pub fn is_bind_cpu(&self) -> bool {
        self.bind_cpu != rt_bindings::RT_CPUS_NR as u8
    }

    #[cfg(feature = "RT_USING_SMP")]
    #[inline]
    pub fn set_bind_cpu(&mut self, cpu_id: u8) {
        debug_assert!(cpu_id < rt_bindings::RT_CPUS_NR as u8);
        self.bind_cpu = cpu_id;
    }

    #[cfg(feature = "RT_USING_SMP")]
    #[inline]
    pub fn get_bind_cpu(&self) -> u8 {
        self.bind_cpu
    }

    #[inline]
    pub fn get_cleanup_fn(&self) -> Option<ThreadCleanupFn> {
        if self.cleanup.is_null() {
            None
        } else {
            unsafe { mem::transmute(self.cleanup) }
        }
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
        unsafe { CStr::from_char_ptr(self.parent.name.as_ptr()) }
    }

    #[inline]
    pub(crate) fn remove_tlist(&mut self) {
        unsafe { Pin::new_unchecked(&mut self.tlist).remove() };
    }

    // thread timeout handler func.
    #[no_mangle]
    extern "C" fn handle_timeout(para: *mut ffi::c_void) {
        debug_assert!(para != ptr::null_mut());
        debug_assert!(
            object::rt_object_get_type(para as rt_bindings::rt_object_t)
                == ObjectClassType::ObjectClassThread as u8
        );

        let thread = unsafe { &mut *(para as *mut RtThread) };
        let scheduler = Cpu::get_current_scheduler();
        let level = scheduler.sched_lock();

        debug_assert!(thread.is_suspended());
        // FIXME
        // thread.error = -(rt_bindings::RT_ETIMEOUT as i32);
        thread.error = -116;
        /* remove from suspend list */
        unsafe { Pin::new_unchecked(&mut thread.tlist).remove() };

        scheduler.insert_thread_locked(thread);

        scheduler.sched_unlock_with_sched(level);
    }

    unsafe extern "C" fn exit() {
        crate::current_thread!().unwrap().as_mut().detach();
    }

    #[inline]
    pub fn handle_tick_increase(&mut self) -> bool {
        debug_assert!(Cpu::get_current_scheduler().is_sched_locked());
        debug_assert!(self.is_current_runnung_thread());
        self.remaining_tick -= 1;
        if self.remaining_tick == 0 {
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
        if self.pending_object != ptr::null_mut()
            && object::rt_object_get_type(self.pending_object)
                == ObjectClassType::ObjectClassMutex as u8
        {
            unsafe {
                rt_bindings::rt_mutex_drop_thread(
                    self.pending_object as *mut rt_bindings::rt_mutex,
                    self as *mut RtThread as *mut rt_bindings::rt_thread,
                )
            };
            self.pending_object = ptr::null_mut();
        }

        // crate::list_for_each_safe!(node, tmp, &self.taken_object_list, {
        //     let mutex = crate::rt_list_entry!(node, rt_bindings::rt_mutex, taken_list);
        //     unsafe { rt_bindings::rt_mutex_release(mutex) };
        // });

        let mut inspect = &self.taken_object_list;
        while let Some(next) = inspect.next() {
            inspect = unsafe { &*next.as_ptr() };
            if core::ptr::eq(inspect, &self.taken_object_list) {
                break;
            }
            unsafe {
                let mutex = crate::rt_list_entry!(
                    inspect as *const ListHead as *mut ListHead,
                    rt_bindings::rt_mutex,
                    taken_list
                );
                rt_bindings::rt_mutex_release(mutex);
            }
        }

        self.spinlock.unlock_irqrestore(level);
    }

    #[inline]
    pub(crate) fn set_priority(&mut self, priority: u8) {
        self.current_priority = priority;

        // FIXME: RT_THREAD_PRIORITY_MAX > 32
        self.number = self.current_priority >> 3; // 5 bits
        self.high_mask = 1 << (self.current_priority & 0x07); // 3 bits
        self.number_mask = 1 << self.number;
        // FIXME: RT_THREAD_PRIORITY_MAX <= 32
        // self.number_mask = 1 << self.current_priority
    }

    pub fn start(&mut self) {
        let scheduler = Cpu::get_current_scheduler();
        let level = scheduler.sched_lock();

        #[cfg(feature = "DEBUG_SCHEDULER")]
        println!("thread start: {:?}", self.get_name());

        self.set_priority(self.current_priority);
        // set to suspend and resume.
        self.stat = rt_bindings::RT_THREAD_SUSPEND as u8;

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

        #[cfg(feature = "DEBUG_SCHEDULER")]
        println!("thread close: {:?}", self.get_name());

        if self.stat != rt_bindings::RT_THREAD_CLOSE as u8 {
            if self.stat != rt_bindings::RT_THREAD_INIT as u8 {
                scheduler.remove_thread_locked(self);
            }

            unsafe { rt_bindings::rt_timer_detach(&mut self.thread_timer) };

            self.stat = rt_bindings::RT_THREAD_CLOSE as u8;
        }

        scheduler.sched_unlock(level);
    }

    pub fn detach(&mut self) {
        // forbid scheduling on current core before returning since current thread
        // may be detached from scheduler.
        let scheduler = Cpu::get_current_scheduler();
        scheduler.preempt_disable();

        #[cfg(feature = "DEBUG_SCHEDULER")]
        println!("thread detach: {:?}", self.get_name());

        self.close();

        #[cfg(feature = "RT_USING_MUTEX")]
        self.detach_from_mutex();

        unsafe { zombie::ZOMBIE_MANAGER.zombie_enqueue(self) };

        scheduler.do_task_schedule();
        scheduler.preempt_enable();
    }

    pub(crate) fn timer_stop(&mut self) -> bool {
        debug_assert!(Cpu::get_current_scheduler().is_sched_locked());
        let mut res = true;
        if self.sched_flag_ttmr_set != 0 {
            res = unsafe {
                rt_bindings::rt_timer_stop(&mut self.thread_timer) == rt_bindings::RT_EOK as i32
            };
            self.sched_flag_ttmr_set = 0;
        }
        res
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
        thread.error = rt_bindings::RT_EOK as i32;

        #[cfg(feature = "DEBUG_SCHEDULER")]
        println!("thread sleep: {:?}", thread.get_name());

        if thread.suspend(rt_bindings::RT_INTERRUPTIBLE) {
            unsafe {
                rt_bindings::rt_timer_control(
                    &mut thread.thread_timer as *mut rt_bindings::rt_timer,
                    rt_bindings::RT_TIMER_CTRL_SET_TIME as i32,
                    &tick as *const _ as *mut ffi::c_void,
                );
                rt_bindings::rt_timer_start(&mut thread.thread_timer as *mut rt_bindings::rt_timer);
            }
            thread.error = -(rt_bindings::RT_EINTR as i32);

            // notify a pending rescheduling
            scheduler.do_task_schedule();
            // exit critical and do a rescheduling
            scheduler.preempt_enable();

            // FIXME
            //if thread.error == -(rt_bindings::RT_ETIMEOUT as i32) {
            if thread.error == -116 {
                thread.error = rt_bindings::RT_EOK as i32;
            }
        }

        Ok(())
    }

    pub(crate) fn suspend(&mut self, suspend_flag: u32) -> bool {
        assert!(self.is_current_runnung_thread());
        let scheduler = Cpu::get_current_scheduler();

        let level = scheduler.sched_lock();

        #[cfg(feature = "DEBUG_SCHEDULER")]
        println!("thread suspend: {:?}", self.get_name());

        if (!self.is_ready()) && (!self.is_running()) {
            println!("thread suspend: thread disorder, stat: {:?}", self.stat);
            scheduler.sched_unlock(level);
            return false;
        }

        // change thread stat
        scheduler.remove_thread_locked(self);
        #[cfg(feature = "RT_USING_SMP")]
        {
            self.oncpu = rt_bindings::RT_CPU_DETACHED as u8;
        }

        let stat = match suspend_flag {
            rt_bindings::RT_INTERRUPTIBLE => rt_bindings::RT_THREAD_SUSPEND_INTERRUPTIBLE,
            rt_bindings::RT_KILLABLE => rt_bindings::RT_THREAD_SUSPEND_KILLABLE,
            rt_bindings::RT_UNINTERRUPTIBLE => rt_bindings::RT_THREAD_SUSPEND_UNINTERRUPTIBLE,
            _ => unreachable!(),
        } as u8;
        self.stat = stat | (self.stat & !(rt_bindings::RT_THREAD_STAT_MASK as u8));
        // stop thread timer anyway
        self.timer_stop();

        scheduler.sched_unlock(level);

        unsafe { crate::rt_object_hook_call!(RT_THREAD_SUSPEND_HOOK, self as *const _ as *mut _) };
        return true;
    }

    pub fn resume(&mut self) -> bool {
        let scheduler = Cpu::get_current_scheduler();

        let level = scheduler.sched_lock();

        #[cfg(feature = "DEBUG_SCHEDULER")]
        println!("thread resume: {:?}", self.get_name());

        let need_schedule = scheduler.insert_ready_locked(self);
        // if need_schedule {
        //     scheduler.sched_unlock_with_sched(level);
        // } else {
        scheduler.sched_unlock(level);
        // }

        unsafe { crate::rt_object_hook_call!(RT_THREAD_RESUME_HOOK, self as *mut _) };
        return need_schedule;
    }

    pub fn change_priority(&mut self, priority: u8) {
        let scheduler = Cpu::get_current_scheduler();
        let level = scheduler.sched_lock();
        if self.is_ready() {
            scheduler.remove_thread_locked(self);
            self.set_priority(priority);
            self.set_init_stat();
            // insert thread to schedule queue again
            scheduler.insert_thread_locked(self);
        } else {
            self.set_priority(priority);
        }
        scheduler.sched_unlock(level);
    }

    #[cfg(feature = "RT_USING_SMP")]
    pub fn bind_to_cpu(&mut self, cpu: u8) {
        let cpu: u8 = if cpu >= rt_bindings::RT_CPUS_NR as u8 {
            rt_bindings::RT_CPUS_NR as u8
        } else {
            cpu
        };

        let scheduler = Cpu::get_current_scheduler();
        let level = scheduler.sched_lock();

        if self.is_ready() {
            scheduler.remove_thread_locked(self);
            self.set_bind_cpu(cpu);
            scheduler.insert_thread_locked(self);
            scheduler.sched_unlock_with_sched(level);
        } else {
            self.bind_cpu = cpu;
            // thread is running on a cpu
            let cur_cpu = scheduler.get_current_id();
            if cpu != rt_bindings::RT_CPUS_NR as u8 {
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

            if let Some(wait_lock) = th.wait_lock.get() {
                owner = unsafe { wait_lock.as_ref().owner.clone() };
            } else {
                break;
            }
        }
        false
    }

    #[cfg(feature = "RT_DEBUGING_SPINLOCK")]
    pub(crate) fn set_wait(&self, spin: &RawSpin) {
        unsafe {
            self.wait_lock
                .set(Some(NonNull::new_unchecked(spin as *const _ as *mut _)))
        };
    }

    #[cfg(feature = "RT_DEBUGING_SPINLOCK")]
    pub(crate) fn clear_wait(&self) {
        self.wait_lock.set(None);
    }
}

#[pinned_drop]
impl PinnedDrop for RtThread {
    fn drop(self: Pin<&mut Self>) {
        let this_th = unsafe { Pin::get_unchecked_mut(self) };

        #[cfg(feature = "DEBUG_SCHEDULER")]
        println!("drop thread: {:?}", this_th.get_name());

        this_th.detach();
    }
}

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
        parameter,
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
    assert!(th_mut.is_init_stat());
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
        parameter,
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

    rt_bindings::RT_EOK as i32
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
    if th.suspend(suspend_flag) {
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

// hooks
#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
#[no_mangle]
pub extern "C" fn rt_thread_suspend_sethook(hook: RtThreadHook) {
    unsafe {
        RT_THREAD_SUSPEND_HOOK = Some(hook);
    }
}

#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
#[no_mangle]
pub extern "C" fn rt_thread_resume_sethook(hook: RtThreadHook) {
    unsafe {
        RT_THREAD_RESUME_HOOK = Some(hook);
    }
}
