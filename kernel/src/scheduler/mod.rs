extern crate alloc;
use crate::{
    arch,
    support::DisableInterruptGuard,
    sync::{SpinLock, SpinLockGuard},
    thread,
    thread::{GlobalQueueVisitor, Thread, ThreadNode},
    time::{timer::Timer, WAITING_FOREVER},
    types::{Arc, IlistHead},
};
use alloc::boxed::Box;
use bluekernel_kconfig::NUM_CORES;
use core::{
    mem::MaybeUninit,
    sync::atomic::{compiler_fence, AtomicBool, Ordering},
};

#[cfg(scheduler = "fifo")]
mod fifo;
#[cfg(scheduler = "global")]
mod global_scheduler;
mod idle;
mod wait_queue;

#[cfg(scheduler = "fifo")]
pub use fifo::*;
#[cfg(scheduler = "global")]
pub use global_scheduler::*;
pub(crate) use wait_queue::*;

pub(crate) static mut RUNNING_THREADS: [MaybeUninit<ThreadNode>; NUM_CORES] =
    [const { MaybeUninit::zeroed() }; NUM_CORES];

pub(crate) fn init() {
    idle::init_idle_threads();
    #[cfg(scheduler = "global")]
    global_scheduler::init();
    #[cfg(scheduler = "fifo")]
    fifo::init();
}

pub(crate) struct ContextSwitchHookHolder<'a> {
    // Next thread is a must.
    pub next_thread: Option<ThreadNode>,
    pub ready_thread: Option<ThreadNode>,
    pub retiring_thread: Option<ThreadNode>,
    pub pending_thread: Option<ThreadNode>,
    pub closure: Option<Box<dyn FnOnce()>>,
    pub dropper: Option<DefaultWaitQueueGuardDropper<'a>>,
}

impl<'a> ContextSwitchHookHolder<'a> {
    pub fn new(next_thread: ThreadNode) -> Self {
        Self {
            next_thread: Some(next_thread),
            ready_thread: None,
            retiring_thread: None,
            pending_thread: None,
            closure: None,
            dropper: None,
        }
    }

    pub fn set_dropper(&mut self, d: DefaultWaitQueueGuardDropper<'a>) -> &mut Self {
        self.dropper = Some(d);
        return self;
    }

    pub fn set_ready_thread(&mut self, t: ThreadNode) -> &mut Self {
        self.ready_thread = Some(t);
        self
    }

    pub fn set_pending_thread(&mut self, t: ThreadNode) -> &mut Self {
        self.pending_thread = Some(t);
        self
    }

    pub fn set_retiring_thread(&mut self, t: ThreadNode) -> &mut Self {
        self.retiring_thread = Some(t);
        self
    }

    pub fn set_closure(&mut self, closure: Box<dyn FnOnce()>) -> &mut Self {
        self.closure = Some(closure);
        self
    }
}

// Must use next's thread's stack or system stack to exeucte this function.
// We assume this hook is invoked with local irq disabled.
// FIXME: rustc miscompiles it if inlined.
#[inline(never)]
pub(crate) extern "C" fn save_context_finish_hook(hook: Option<&mut ContextSwitchHookHolder>) {
    let Some(hook) = hook else {
        return;
    };
    // We must be careful that the last use of the `hook` must
    // happen-before enqueueing the ready thread to the ready queue,
    // since the `hook` is still on the stack of the ready thread. To
    // resolve race condition of the stack, we first take the
    // ownership of all pending actions in the `hook`, so that these
    // actions are on current stack.
    let ready_thread = hook.ready_thread.take();
    let retiring_thread = hook.retiring_thread.take();
    let closure = hook.closure.take();
    let pending_thread = hook.pending_thread.take();
    let mut dropper = hook.dropper.take();
    let next = hook.next_thread.take();
    compiler_fence(Ordering::SeqCst);
    let Some(next) = next else {
        panic!("Next thread must be specified!");
    };
    {
        let ok = next.transfer_state(thread::READY, thread::RUNNING);
        assert!(ok);
        let old = set_current_thread(next.clone());
        #[cfg(debugging_scheduler)]
        crate::trace!(
            "Switching from 0x{:x}: 0x{:x} to 0x{:x}: 0x{:x}",
            Thread::id(&old),
            old.saved_sp(),
            Thread::id(&next),
            next.saved_sp(),
        );
    }
    compiler_fence(Ordering::SeqCst);
    ready_thread.map(|t| {
        let ok = crate::scheduler::queue_ready_thread(thread::RUNNING, t);
        assert!(ok);
    });
    compiler_fence(Ordering::SeqCst);
    pending_thread.map(|t| {
        let ok = t.transfer_state(thread::RUNNING, thread::SUSPENDED);
        assert!(ok);
    });
    compiler_fence(Ordering::SeqCst);
    // Local irq is disabled by arch and the scheduler assumes every
    // thread should be resumed with local irq enabled.
    dropper.as_mut().map(|v| v.forget_irq());
    drop(dropper);
    compiler_fence(Ordering::SeqCst);
    closure.map(|f| f());
    compiler_fence(Ordering::SeqCst);
    retiring_thread.map(|t| {
        let ok = t.transfer_state(thread::RUNNING, thread::RETIRED);
        assert!(ok);
        GlobalQueueVisitor::remove(&t);
        if ThreadNode::strong_count(&t) != 1 {
            // TODO: Warn if there are still references to the thread.
        }
    });
}

// It's usually used in cortex-m's pendsv handler. It assumes current
// thread's context is already saved.
pub(crate) extern "C" fn yield_me_and_return_next_sp(old_sp: usize) -> usize {
    let dig = DisableInterruptGuard::new();
    let Some(next) = next_ready_thread() else {
        #[cfg(debugging_scheduler)]
        crate::trace!("0x{:x} keeps running", Thread::id(&current_thread()));

        return old_sp;
    };
    let to_sp = next.saved_sp();
    let ok = next.transfer_state(thread::READY, thread::RUNNING);
    assert!(ok);
    let old = set_current_thread(next.clone());
    #[cfg(debugging_scheduler)]
    crate::trace!(
        "Switching from 0x{:x}: 0x{:x} to 0x{:x}: 0x{:x}",
        Thread::id(&old),
        old.saved_sp(),
        Thread::id(&next),
        next.saved_sp(),
    );
    old.lock().set_saved_sp(old_sp);
    let ok = queue_ready_thread(thread::RUNNING, old);
    assert!(ok);
    return to_sp;
}

pub fn retire_me() -> ! {
    let next = next_ready_thread().map_or_else(|| idle::current_idle_thread().clone(), |v| v);
    let to_sp = next.saved_sp();
    // FIXME: Some WaitQueue might still share the ownership of
    // the `old`, shall we record which WaitQueue the `old`
    // belongs to? Weak reference might not help to reduce memory
    // usage.
    let mut hooks = ContextSwitchHookHolder::new(next);
    hooks.set_retiring_thread(current_thread());
    arch::restore_context_with_hook(to_sp, &mut hooks as *mut _);
}

pub fn yield_me() {
    // We don't allow thread yielding with irq disabled.
    // The scheduler assumes every thread should be resumed with local
    // irq enabled.
    assert!(arch::local_irq_enabled());
    let pg = thread::Thread::try_preempt_me();
    if !pg.preemptable() {
        arch::idle();
        return;
    }
    drop(pg);
    yield_unconditionally();
}

fn yield_unconditionally() {
    assert!(arch::local_irq_enabled());
    let Some(next) = next_ready_thread() else {
        arch::idle();
        return;
    };
    let to_sp = next.saved_sp();
    let old = current_thread();
    let from_sp_ptr = old.saved_sp_ptr();
    let mut hook_holder = ContextSwitchHookHolder::new(next);
    if Thread::id(&old) == Thread::id(idle::current_idle_thread()) {
        let ok = old.transfer_state(thread::RUNNING, thread::READY);
        assert!(ok);
        drop(old);
        // We should never put idle thread to ready queue.
        arch::switch_context_with_hook(from_sp_ptr as *mut u8, to_sp, &mut hook_holder as *mut _);
    } else {
        hook_holder.set_ready_thread(old);
        arch::switch_context_with_hook(from_sp_ptr as *mut u8, to_sp, &mut hook_holder as *mut _);
    }
    assert!(arch::local_irq_enabled());
}

pub(crate) fn suspend_me_with_hook(hook: impl FnOnce() + 'static) {
    let next = next_ready_thread().map_or_else(|| idle::current_idle_thread().clone(), |v| v);
    let to_sp = next.saved_sp();
    let old = current_thread();
    let from_sp_ptr = old.saved_sp_ptr();
    let mut hook_holder = ContextSwitchHookHolder::new(next);
    let hook = Box::new(hook);
    hook_holder.set_closure(hook);
    arch::switch_context_with_hook(from_sp_ptr as *mut u8, to_sp, &mut hook_holder as *mut _);
    assert!(arch::local_irq_enabled());
}

// Sometimes we need to queue the thread into two queues
// simultaneously, like semaphore with timeout.
pub(crate) fn suspend_me_2<'a>(
    mut w0: SpinLockGuard<'a, WaitQueue>,
    mut w1: SpinLockGuard<'a, WaitQueue>,
) {
    let next = next_ready_thread().map_or_else(|| idle::current_idle_thread().clone(), |v| v);
    let to_sp = next.saved_sp();
    let old = current_thread();
    let from_sp_ptr = old.saved_sp_ptr();
    w0.push_back(Arc::new(WaitEntry {
        wait_node: IlistHead::<WaitEntry, OffsetOfWait>::new(),
        thread: old.clone(),
    }));
    w1.push_back(Arc::new(WaitEntry {
        wait_node: IlistHead::<WaitEntry, OffsetOfWait>::new(),
        thread: old.clone(),
    }));
    let mut dropper = DefaultWaitQueueGuardDropper::new();
    // Add them in reversed order.
    dropper.add(w1);
    dropper.add(w0);
    let mut hook_holder = ContextSwitchHookHolder::new(next);
    hook_holder.set_dropper(dropper);
    hook_holder.set_pending_thread(old);
    arch::switch_context_with_hook(from_sp_ptr as *mut u8, to_sp, &mut hook_holder as *mut _);
    assert!(arch::local_irq_enabled());
}

pub(crate) fn suspend_me_for(tick: usize) {
    assert!(tick != 0);
    let next = next_ready_thread().map_or_else(|| idle::current_idle_thread().clone(), |v| v);
    let to_sp = next.saved_sp();
    let old = current_thread();
    let from_sp_ptr = old.saved_sp_ptr();
    let mut hook_holder = ContextSwitchHookHolder::new(next);
    hook_holder.set_pending_thread(old.clone());

    if tick != WAITING_FOREVER {
        let th = old.clone();
        let timer_callback = Box::new(move || {
            #[cfg(debugging_scheduler)]
            crate::trace!("add thread to ready queue after timeout");
            let _ = queue_ready_thread(thread::SUSPENDED, th.clone());
        });
        let hook = Box::new(move || {
            match &old.timer {
                Some(t) => {
                    t.set_callback(timer_callback);
                    t.start_new_interval(tick);
                }
                None => {
                    let timer = Timer::new_hard_oneshot(tick, timer_callback);
                    old.lock().timer = Some(timer.clone());
                    compiler_fence(Ordering::SeqCst);
                    timer.start();
                }
            };
        });
        hook_holder.set_closure(hook);
    }

    arch::switch_context_with_hook(from_sp_ptr as *mut u8, to_sp, &mut hook_holder as *mut _);
    assert!(arch::local_irq_enabled());
}

pub(crate) fn suspend_me_timed_wait<'a>(mut w: SpinLockGuard<'a, WaitQueue>, tick: usize) -> bool {
    assert!(tick != 0);
    let next = next_ready_thread().map_or_else(|| idle::current_idle_thread().clone(), |v| v);
    // FIXME: Ideally, we should defer state transfer to context switch hook.
    let to_sp = next.saved_sp();
    let old = current_thread();
    let from_sp_ptr = old.saved_sp_ptr();
    let entry = Arc::new(WaitEntry {
        wait_node: IlistHead::<WaitEntry, OffsetOfWait>::new(),
        thread: old.clone(),
    });
    let ok = w.push_back(entry);
    assert!(ok);
    // old's context saving must happen before old is requeued to
    // ready queue.
    // Ideally, we need an API like
    // ```
    // switch_context(from_sp_mut, to_sp, w)
    // ```
    // which is hard to implement in Rust. So we wrap all guards
    // inside a hook hodler and pass it by its
    // pointer. save_context_finish_hook is called during
    // switching context.
    let mut dropper = DefaultWaitQueueGuardDropper::new();
    dropper.add(w);
    let mut hook_holder = ContextSwitchHookHolder::new(next);
    hook_holder.set_dropper(dropper);
    hook_holder.set_pending_thread(old.clone());
    let timed_out = Arc::new(AtomicBool::new(false));
    let timed_out_clone = timed_out.clone();

    if tick != WAITING_FOREVER {
        let th = old.clone();
        let timer_callback = Box::new(move || {
            #[cfg(debugging_scheduler)]
            crate::trace!("add thread to ready queue after timeout");
            let _ = queue_ready_thread(thread::SUSPENDED, th.clone());
            timed_out_clone.store(true, Ordering::SeqCst);
        });
        let hook = Box::new(move || {
            match &old.timer {
                Some(t) => {
                    t.set_callback(timer_callback);
                    t.start_new_interval(tick);
                }
                None => {
                    let timer = Timer::new_hard_oneshot(tick, timer_callback);
                    old.lock().timer = Some(timer.clone());
                    compiler_fence(Ordering::SeqCst);
                    timer.start();
                }
            };
        });
        hook_holder.set_closure(hook);
    }

    arch::switch_context_with_hook(from_sp_ptr as *mut u8, to_sp, &mut hook_holder as *mut _);
    assert!(arch::local_irq_enabled());
    return timed_out.load(Ordering::SeqCst);
}

// Yield me immediately if not in ISR, otherwise switch context on
// exiting of the inner most ISR. Or just do nothing if underling arch
// doesn't have good support of this semantics. Cortex-m's pendsv is
// perfectly meet this semantics.
pub(crate) fn yield_me_now_or_later() {
    arch::pend_switch_context();
}

// Entry of system idle threads.
pub(crate) extern "C" fn schedule() -> ! {
    #[cfg(debugging_scheduler)]
    crate::trace!("Start scheduling");

    arch::enable_local_irq();
    assert!(arch::local_irq_enabled());
    loop {
        yield_me();
    }
}

#[inline]
pub fn current_thread() -> ThreadNode {
    let _ = DisableInterruptGuard::new();
    let my_id = arch::current_cpu_id();
    let t = unsafe { RUNNING_THREADS[my_id].assume_init_ref().clone() };
    return t;
}

pub(crate) fn handle_tick_increment(escape_tick: usize) -> bool {
    #[cfg(robin_scheduler)]
    {
        let th = current_thread();
        if Thread::id(&th) != Thread::id(idle::current_idle_thread())
            && th.round_robin(escape_tick) <= 0
            && th.is_preemptable()
        {
            th.reset_robin();
            return true;
        }
    }
    false
}

fn set_current_thread(t: ThreadNode) -> ThreadNode {
    let _dig = DisableInterruptGuard::new();
    let my_id = arch::current_cpu_id();
    assert!(t.validate_saved_sp());
    let old = unsafe { core::mem::replace(RUNNING_THREADS[my_id].assume_init_mut(), t) };
    // Do not validate sp here, since we might be using system stack,
    // like on cortex-m platform.
    return old;
}
