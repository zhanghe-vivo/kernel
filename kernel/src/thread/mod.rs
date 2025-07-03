extern crate alloc;
use crate::{
    arch, config, debug, scheduler,
    support::{Region, RegionalObjectBuilder},
    time::timer::Timer,
    types::{
        impl_simple_intrusive_adapter, Arc, AtomicUint, IRwLock, IlistHead,
        RwLockWriteGuard as WriteGuard, ThreadPriority, Uint,
    },
};
use alloc::boxed::Box;
use core::sync::atomic::{AtomicI32, AtomicUsize, Ordering};

mod builder;
mod posix;
pub use builder::*;
use posix::*;

pub type ThreadNode = Arc<Thread>;

pub enum Entry {
    C(extern "C" fn()),
    Posix(
        extern "C" fn(*mut core::ffi::c_void),
        *mut core::ffi::c_void,
    ),
    Closure(Box<dyn FnOnce()>),
}

impl core::fmt::Debug for Entry {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        Ok(())
    }
}

#[derive(Debug, Copy, Clone)]
pub enum ThreadKind {
    AsyncPoller,
    Idle,
    Normal,
    #[cfg(soft_timer)]
    SoftTimer,
}

impl Default for ThreadKind {
    fn default() -> Self {
        ThreadKind::Normal
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(align(16))]
pub struct AlignedStackStorage([u8; config::DEFAULT_STACK_SIZE]);

#[derive(Debug)]
pub enum Stack {
    Raw { base: usize, size: usize },
    Boxed(Box<AlignedStackStorage>),
}

impl Default for Stack {
    fn default() -> Self {
        Stack::Raw { base: 0, size: 0 }
    }
}

impl Stack {
    pub fn base(&self) -> usize {
        match self {
            Self::Boxed(ref boxed) => boxed.0.as_ptr() as usize,
            Self::Raw { base, .. } => *base,
        }
    }

    pub fn size(&self) -> usize {
        match self {
            Self::Boxed(ref boxed) => boxed.0.len(),
            Self::Raw { size, .. } => *size,
        }
    }
}

impl_simple_intrusive_adapter!(OffsetOfSchedNode, Thread, sched_node);
impl_simple_intrusive_adapter!(OffsetOfGlobal, Thread, global);
impl_simple_intrusive_adapter!(OffsetOfLock, Thread, lock);

pub const CREATED: Uint = 0;
pub const READY: Uint = 1;
pub const RUNNING: Uint = 2;
pub const SUSPENDED: Uint = 3;
pub const RETIRED: Uint = 4;

// ThreadStats is protected by thread scheduler.
#[derive(Debug, Default)]
pub struct ThreadStats {
    start: u64,
    cycles: u64,
}

impl ThreadStats {
    pub const fn new() -> Self {
        Self {
            start: 0,
            cycles: 0,
        }
    }

    pub fn increment_cycles(&mut self, cycles: u64) {
        self.cycles += cycles.saturating_sub(self.start);
    }

    pub fn set_start_cycles(&mut self, start: u64) {
        self.start = start;
    }

    pub fn get_cycles(&self) -> u64 {
        self.cycles
    }
}

#[derive(Default, Debug)]
pub struct Thread {
    pub global: IlistHead<Thread, OffsetOfGlobal>,
    pub sched_node: IlistHead<Thread, OffsetOfSchedNode>,
    pub timer: Option<Arc<Timer>>,
    // Cleanup function will be invoked when retiring.
    cleanup: Option<Entry>,
    kind: ThreadKind,
    stack: Stack,
    saved_sp: usize,
    priority: ThreadPriority,
    state: AtomicUint,
    preempt_count: AtomicUint,
    #[cfg(robin_scheduler)]
    robin_count: AtomicI32,
    // FIXME: Using a rusty lock looks not flexible. Now we are using
    // a C-style intrusive lock. It's conventional to declare which
    // fields this lock is protecting. lock is protecting the
    // whole struct except those atomic fields.
    lock: IRwLock<Thread, OffsetOfLock>,
    posix_compat: Option<PosixCompat>,
    stats: ThreadStats,
}

extern "C" fn run_simple_c(f: extern "C" fn()) {
    f();
    scheduler::retire_me();
}

extern "C" fn run_posix(f: extern "C" fn(*mut core::ffi::c_void), arg: *mut core::ffi::c_void) {
    f(arg);
    scheduler::retire_me();
}

// FIXME: If the closure doesn't get run, memory leaks.
extern "C" fn run_closure(raw: *mut Box<dyn FnOnce()>) {
    unsafe { Box::from_raw(raw)() };
    scheduler::retire_me();
}

impl Thread {
    #[inline]
    pub fn stats(&self) -> &ThreadStats {
        &self.stats
    }

    // FIXME: rustc miscompiles it if not inlined.
    #[inline]
    pub fn lock(&self) -> WriteGuard<'_, Self> {
        self.lock.write()
    }

    #[inline(always)]
    pub fn stack_usage(&self) -> usize {
        let sp = arch::current_sp();
        let used = self.stack.base() + self.stack.size() - sp;
        return used;
    }

    #[inline(always)]
    pub fn validate_sp(&self) -> bool {
        let sp = arch::current_sp();
        return sp >= self.stack.base() && sp <= self.stack.base() + self.stack.size();
    }

    #[inline(always)]
    pub fn validate_saved_sp(&self) -> bool {
        let sp = self.saved_sp;
        return sp >= self.stack.base() && sp <= self.stack.base() + self.stack.size();
    }

    #[inline(always)]
    pub fn saved_stack_usage(&self) -> usize {
        return self.stack.base() + self.stack.size() - self.saved_sp();
    }

    #[inline]
    pub fn stack_base(&self) -> usize {
        self.stack.base()
    }

    #[inline]
    pub fn stack_size(&self) -> usize {
        self.stack.size()
    }

    #[inline]
    pub fn state(&self) -> Uint {
        self.state.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn state_to_str(&self) -> &str {
        let state = self.state.load(Ordering::Relaxed);
        match state {
            CREATED => "created",
            READY => "ready",
            RUNNING => "running",
            SUSPENDED => "suspended",
            RETIRED => "retired",
            _ => "unknown",
        }
    }

    #[inline]
    pub fn kind(&self) -> ThreadKind {
        self.kind
    }

    #[inline]
    pub fn kind_to_str(&self) -> &str {
        match self.kind {
            ThreadKind::AsyncPoller => "async_poller",
            ThreadKind::Idle => "idle",
            ThreadKind::Normal => "normal",
            #[cfg(soft_timer)]
            ThreadKind::SoftTimer => "soft_timer",
        }
    }

    #[inline]
    pub fn transfer_state(&self, from: Uint, to: Uint) -> bool {
        self.state
            .compare_exchange(from, to, Ordering::SeqCst, Ordering::Relaxed)
            .is_ok()
    }

    #[inline]
    pub unsafe fn set_state(&self, to: Uint) -> &Self {
        self.state.store(to, Ordering::Relaxed);
        return self;
    }

    #[inline]
    pub fn set_priority(&mut self, p: ThreadPriority) -> &mut Self {
        self.priority = p;
        self
    }

    #[inline]
    pub fn set_kind(&mut self, kind: ThreadKind) -> &mut Self {
        self.kind = kind;
        self
    }

    #[inline]
    pub fn saved_sp_ptr(&self) -> *const u8 {
        &self.saved_sp as *const _ as *const u8
    }

    #[inline]
    pub fn saved_sp(&self) -> usize {
        self.saved_sp
    }

    #[inline]
    pub fn set_stack(&mut self, stack: Stack) -> &mut Self {
        self.stack = stack;
        self
    }

    const fn new(kind: ThreadKind) -> Self {
        Self::const_new(kind)
    }

    #[inline]
    pub fn take_cleanup(&mut self) -> Option<Entry> {
        self.cleanup.take()
    }

    #[inline]
    pub fn set_cleanup(&mut self, cleanup: Entry) {
        self.cleanup = Some(cleanup);
    }

    const fn const_new(kind: ThreadKind) -> Self {
        Self {
            cleanup: None,
            stack: Stack::Raw { base: 0, size: 0 },
            state: AtomicUint::new(CREATED),
            lock: IRwLock::<Thread, OffsetOfLock>::new(),
            sched_node: IlistHead::<Thread, OffsetOfSchedNode>::new(),
            global: IlistHead::<Thread, OffsetOfGlobal>::new(),
            saved_sp: 0,
            priority: 0,
            preempt_count: AtomicUint::new(0),
            posix_compat: None,
            stats: ThreadStats::new(),
            timer: None,
            #[cfg(robin_scheduler)]
            robin_count: AtomicI32::new(0),
            kind,
        }
    }

    #[inline]
    pub fn id(me: &ThreadNode) -> usize {
        unsafe { ThreadNode::get_handle(me) as usize }
    }

    #[inline]
    pub(crate) fn try_preempt_me() -> PreemptGuard {
        let current = scheduler::current_thread();
        let status = current.disable_preempt();
        PreemptGuard { t: current, status }
    }

    #[inline]
    pub(crate) fn try_preempt(t: &ThreadNode) -> PreemptGuard {
        PreemptGuard {
            t: t.clone(),
            status: t.disable_preempt(),
        }
    }

    #[inline]
    pub fn is_preemptable(&self) -> bool {
        self.preempt_count.load(Ordering::Relaxed) == 0
    }

    #[inline]
    pub(crate) fn reset_saved_sp(&mut self) -> &mut Self {
        self.saved_sp = self.stack.base() + self.stack.size();
        return self;
    }

    #[inline]
    pub(crate) fn set_saved_sp(&mut self, sp: usize) -> &mut Self {
        self.saved_sp = sp;
        return self;
    }

    pub(crate) fn init(&mut self, stack: Stack, entry: Entry) -> &mut Self {
        self.stack = stack;
        // TODO: Stack sanity check.
        self.saved_sp =
            self.stack.base() + self.stack.size() - core::mem::size_of::<arch::Context>();
        let region = Region {
            base: self.saved_sp,
            size: core::mem::size_of::<arch::Context>(),
        };
        let mut builder = RegionalObjectBuilder::new(region);
        let ctx = unsafe {
            builder
                .zeroed_after_start::<arch::Context>()
                .unwrap_unchecked()
        };
        assert_eq!(ctx as *const _ as usize, self.saved_sp);
        ctx.init();
        // TODO: We should provide the thread a more rusty environment
        // to run the function safely.
        match entry {
            Entry::C(f) => ctx
                .set_return_address(run_simple_c as usize)
                .set_arg(0, unsafe { core::mem::transmute(f) }),
            Entry::Closure(boxed) => {
                // FIXME: We need to make a new box to contain Box<dyn
                // FnOnce() + Send + 'static>, since *mut (dyn
                // FnOnce() + Send + 'static) is 64 bits in 32-bit
                // platform, aka, it's a fat pointer.
                let raw = Box::into_raw(Box::new(boxed));
                ctx.set_return_address(run_closure as usize)
                    .set_arg(0, raw as *mut u8 as usize)
            }
            Entry::Posix(f, arg) => ctx
                .set_return_address(run_posix as usize)
                .set_arg(0, unsafe { core::mem::transmute(f) })
                .set_arg(1, unsafe { core::mem::transmute(arg) }),
        };
        return self;
    }

    #[inline]
    pub fn priority(&self) -> ThreadPriority {
        self.priority
    }

    #[inline]
    pub fn disable_preempt(&self) -> bool {
        return self.preempt_count.fetch_add(1, Ordering::Acquire) == 0;
    }

    #[inline]
    pub fn enable_preempt(&self) -> bool {
        return self.preempt_count.fetch_sub(1, Ordering::Acquire) == 1;
    }

    #[inline]
    pub fn preempt_count(&self) -> Uint {
        self.preempt_count.load(Ordering::Relaxed)
    }

    #[cfg(robin_scheduler)]
    #[inline]
    pub fn round_robin(&self, tick: usize) -> i32 {
        self.robin_count.fetch_sub(tick as i32, Ordering::Relaxed)
    }

    #[cfg(robin_scheduler)]
    #[inline]
    pub fn reset_robin(&self) {
        self.robin_count
            .store(blueos_kconfig::ROBIN_SLICE as i32, Ordering::Relaxed);
    }

    #[inline]
    pub fn increment_cycles(&mut self, cycles: u64) {
        self.stats.increment_cycles(cycles);
    }

    #[inline]
    pub fn set_start_cycles(&mut self, cycles: u64) {
        self.stats.set_start_cycles(cycles);
    }

    #[inline]
    pub fn get_cycles(&self) -> u64 {
        self.stats.get_cycles()
    }
}

impl Drop for Thread {
    fn drop(&mut self) {
        assert!(self.sched_node.is_detached());
    }
}

pub(crate) struct PreemptGuard {
    t: ThreadNode,
    pub status: bool,
}

impl PreemptGuard {
    #[inline(always)]
    pub fn preemptable(&self) -> bool {
        self.status
    }
}

impl<'a> Drop for PreemptGuard {
    #[inline]
    fn drop(&mut self) {
        self.t.enable_preempt();
    }
}

impl !Send for Thread {}
unsafe impl Sync for Thread {}
