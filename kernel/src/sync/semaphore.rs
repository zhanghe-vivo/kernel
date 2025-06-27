use super::SpinLock;
use crate::{
    arch, debug, scheduler, scheduler::WaitQueue, thread, thread::Thread, trace, types::Int,
};
use core::cell::Cell;

#[derive(Debug)]
pub struct Semaphore {
    counter: Cell<Int>,
    // We let the RwLock protect the whole semaphore.
    pending: SpinLock<WaitQueue>,
}

impl Semaphore {
    pub const fn const_new(counter: Int) -> Self {
        debug_assert!(counter >= 1, "Init resources should not be zero");
        Self {
            counter: Cell::new(counter),
            pending: SpinLock::new(WaitQueue::new()),
        }
    }

    pub const fn new(counter: Int) -> Self {
        Self::const_new(counter)
    }

    pub fn init(&self) -> bool {
        return self.pending.lock().init();
    }

    #[inline(never)]
    pub fn acquire_notimeout(&self) -> bool {
        // We should not wait in IRQ.
        let mut w = self.pending.lock();
        loop {
            let old = self.counter.get();
            debug!(
                "[C#{}:0x{:x}] reads counter to acquire: {}",
                arch::current_cpu_id(),
                Thread::id(&scheduler::current_thread()),
                old,
            );
            if old == 0 {
                scheduler::suspend_me(w);
                w = self.pending.lock();
                continue;
            } else {
                self.counter.set(old - 1);
                break;
            }
        }
        return true;
    }

    pub fn acquire_timeout(&self, _t: u64) -> bool {
        false
    }

    pub fn acquire(&self, timeout: Option<u64>) -> bool {
        let Some(t) = timeout else {
            return self.acquire_notimeout();
        };
        return self.acquire_timeout(t);
    }

    #[inline(never)]
    pub fn release(&self) {
        let mut w = self.pending.irqsave_lock();
        let old = self.counter.get();
        debug!(
            "[C#{}:0x{:x}] reads counter to release: {}",
            arch::current_cpu_id(),
            Thread::id(&scheduler::current_thread()),
            old,
        );
        self.counter.set(old + 1);
        if old > 0 {
            return;
        }
        while let Some(next) = w.pop_front() {
            let t = next.thread.clone();
            let ok = scheduler::queue_ready_thread(thread::SUSPENDED, t);
            if ok {
                break;
            }
            trace!(
                "Failed to enqueue 0x{:x}, state: {}",
                Thread::id(&next.thread),
                next.thread.state()
            );
        }
        drop(w);
        scheduler::yield_me_now_or_later();
    }
}

impl !Send for Semaphore {}
unsafe impl Sync for Semaphore {}
