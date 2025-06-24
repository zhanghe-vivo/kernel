use crate::{
    arch, boards, config, config::SOFT_TIMER_THREAD_PRIORITY, scheduler, sync, thread,
    types::impl_simple_intrusive_adapter,
};
use core::mem::MaybeUninit;
use scheduler::WaitQueue;
use sync::spinlock::SpinLock;
use thread::{Entry, SystemThreadStorage, ThreadNode};

const TIMER_WHEEL_SIZE: usize = 32;
static SOFT_TIMER_WHEEL: SpinLock<TimerWheel> = SpinLock::new(TimerWheel::new());
static mut SOFT_TIMER_THREAD: MaybeUninit<ThreadNode> = MaybeUninit::zeroed();
static SOFT_TIMER_THREAD_STACK: SystemThreadStorage = SystemThreadStorage::new();
static HARD_TIMER_WHEEL: SpinLock<TimerWheel> = SpinLock::new(TimerWheel::new());

fn init_soft_timer() {
    let mut w = SOFT_TIMER_WHEEL.lock();
    for i in 0..w.wheel.len() {
        let ok = w.wheel[i].init();
        debug_assert!(ok);
    }
    let node = thread::build_static_thread(
        unsafe { &mut SOFT_TIMER_THREAD },
        &SOFT_TIMER_THREAD_STACK,
        config::SOFT_TIMER_THREAD_PRIORITY,
        thread::CREATED,
        Entry::C(run_soft_timer),
    );
    //    let ok = scheduler::queue_ready_thread(thread::CREATED, node);
    //    debug_assert!(ok);
}

extern "C" fn run_soft_timer() {
    loop {
        let mut l = SOFT_TIMER_WHEEL.lock();
        let ct = boards::current_ticks();
        core::hint::spin_loop();
        arch::idle();
    }
}

fn init_hard_timer() {
    let mut w = HARD_TIMER_WHEEL.lock();
    for i in 0..w.wheel.len() {
        let ok = w.wheel[i].init();
        debug_assert!(ok);
    }
}

pub(crate) fn init_timers() {
    //init_soft_timer();
    //init_hard_timer();
}

pub struct TimerWheel {
    wheel: [WaitQueue; TIMER_WHEEL_SIZE],
    cursor: usize,
}

impl TimerWheel {
    pub const fn new() -> Self {
        Self {
            wheel: [const { WaitQueue::new() }; TIMER_WHEEL_SIZE],
            cursor: 0,
        }
    }
}
