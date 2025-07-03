extern crate alloc;
use crate::{
    arch,
    config::MAX_THREAD_PRIORITY,
    scheduler::RUNNING_THREADS,
    support,
    thread::{self, Entry, SystemThreadStorage, Thread, ThreadKind, ThreadNode},
};
use blueos_kconfig::NUM_CORES;
use core::mem::MaybeUninit;

static IDLE_THREAD_BLOCKS: [SystemThreadStorage; NUM_CORES] =
    [const { SystemThreadStorage::new(ThreadKind::Idle) }; NUM_CORES];
static mut IDLE_THREADS: [MaybeUninit<ThreadNode>; NUM_CORES] =
    [const { MaybeUninit::zeroed() }; NUM_CORES];

extern "C" fn fake_idle_thread_entry() {
    unreachable!("Should use real entry specified in start_schedule");
}

fn init_idle_thread(i: usize) {
    let arc = thread::build_static_thread(
        unsafe { &mut IDLE_THREADS[i] },
        &IDLE_THREAD_BLOCKS[i],
        MAX_THREAD_PRIORITY,
        thread::RUNNING,
        Entry::C(fake_idle_thread_entry),
        ThreadKind::Idle,
    );
    unsafe {
        RUNNING_THREADS[i].write(arc.clone());
    }
}

pub(super) fn init_idle_threads() {
    for i in 0..NUM_CORES {
        init_idle_thread(i);
    }
}

#[inline]
pub(super) fn current_idle_thread<'a>() -> &'a ThreadNode {
    let _dig = support::DisableInterruptGuard::new();
    let id = arch::current_cpu_id();
    return unsafe { IDLE_THREADS[id].assume_init_ref() };
}

#[inline]
pub fn get_idle_thread<'a>(cpu_id: usize) -> &'a ThreadNode {
    let _dig = support::DisableInterruptGuard::new();
    return unsafe { IDLE_THREADS[cpu_id].assume_init_ref() };
}
