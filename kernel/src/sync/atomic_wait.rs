extern crate alloc;
use crate::{
    arch, debug,
    error::{code, Error},
    scheduler, static_arc, support,
    sync::SpinLock,
    thread,
    thread::{Thread, ThreadNode},
    time::WAITING_FOREVER,
    trace,
    types::{impl_simple_intrusive_adapter, Arc, ArcList, ArcListIterator, IlistHead as ListHead},
};
use core::sync::atomic::{AtomicUsize, Ordering};
use scheduler::WaitQueue;
use support::PerCpu;

impl_simple_intrusive_adapter!(Sync, AtomicWaitEntry, sync_node);

type Head = ListHead<AtomicWaitEntry, Sync>;
type EntryList = ArcList<AtomicWaitEntry, Sync>;
type EntryNode = Arc<AtomicWaitEntry>;

#[derive(Debug)]
pub struct AtomicWaitEntry {
    pub sync_node: EntryList,
    addr: usize,
    pending: SpinLock<WaitQueue>,
}

impl AtomicWaitEntry {
    pub fn init(&self) -> bool {
        return self.pending.irqsave_lock().init();
    }

    pub fn addr(&self) -> usize {
        self.addr
    }

    pub fn new(addr: usize) -> Self {
        Self {
            sync_node: EntryList::new(),
            addr,
            pending: SpinLock::new(WaitQueue::new()),
        }
    }
}

static_arc! {
    SYNC_ENTRIES(SpinLock<Head>, SpinLock::new(Head::new())),
}

pub fn atomic_wait(addr: usize, val: usize, timeout: Option<usize>) -> Result<(), Error> {
    let ptr = addr as *const AtomicUsize;
    let fetched = unsafe { &*ptr }.load(Ordering::Acquire);
    if fetched != val {
        return Err(code::EAGAIN);
    }
    // We should not wait in IRQ.
    let mut w = SYNC_ENTRIES.irqsave_lock();
    // Make the second check.
    let fetched = unsafe { &*ptr }.load(Ordering::Acquire);
    if fetched != val {
        return Err(code::EAGAIN);
    }
    let mut entry = None;
    for e in ArcListIterator::new(&*w, None) {
        if e.addr() == addr {
            entry = Some(e);
            break;
        }
    }
    let entry = entry.map_or_else(
        || {
            let entry = Arc::new(AtomicWaitEntry::new(addr));
            entry.init();
            EntryList::insert_after(&mut *w, entry.clone());
            return entry;
        },
        |e| e,
    );
    let t = scheduler::current_thread();
    debug!(
        "[C#{}:0x{:x}] Before woken by someone @ 0x{:x}, thread rc: {}, entry rc: {}, stack usage: {}, {:?}",
        arch::current_cpu_id(),
        Thread::id(&t),
        addr,
        ThreadNode::strong_count(&t),
        EntryNode::strong_count(&entry),
        t.stack_usage(),
        *t,
    );
    let we = entry.pending.irqsave_lock();
    w.forget_irq();
    drop(w);
    if let Some(timeout) = timeout {
        let res = scheduler::suspend_me_timed_wait(we, timeout);
        if res == true {
            return Err(code::ETIMEDOUT);
        }
    } else {
        let _ = scheduler::suspend_me_timed_wait(we, WAITING_FOREVER);
    }
    debug!(
        "Woken by someone @ 0x{:x}, entry rc: {}",
        addr,
        EntryNode::strong_count(&entry)
    );
    return Ok(());
}

#[inline(never)]
pub fn atomic_wake(addr: usize, how_many: usize) -> Result<usize, ()> {
    if how_many == 0 {
        return Ok(0);
    }
    let mut woken = 0;
    let w = SYNC_ENTRIES.irqsave_lock();
    for e in ArcListIterator::new(&*w, None) {
        if e.addr() != addr {
            continue;
        }
        let mut we = e.pending.irqsave_lock();
        while let Some(next) = we.pop_front() {
            if scheduler::queue_ready_thread(thread::SUSPENDED, next.thread.clone()) {
                woken += 1;
            }
            if woken == how_many {
                break;
            }
        }
        if we.is_empty() {
            EntryList::detach(&mut e.clone());
        }
        if woken == how_many {
            break;
        }
    }
    drop(w);
    scheduler::yield_me_now_or_later();
    return Ok(woken);
}

#[cfg(cortex_m)]
#[cfg(test)]
mod tests {
    use super::*;
    use bluekernel_test_macro::test;
    use core::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_atomic_wait_timeout() {
        let atomic_var = AtomicUsize::new(0);
        let addr = &atomic_var as *const AtomicUsize as usize;
        let result = atomic_wait(addr, 0, Some(10));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), code::ETIMEDOUT);
    }
}
