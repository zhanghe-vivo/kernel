use crate::{
    config::MAX_THREAD_PRIORITY,
    sync::spinlock::SpinLock,
    thread,
    thread::{Thread, ThreadNode},
    types::{ArcList, ThreadPriority, Uint},
};
use core::mem::MaybeUninit;

static mut READY_TABLE: MaybeUninit<SpinLock<ReadyTable>> = MaybeUninit::zeroed();

type ReadyTableBitFields = u32;

pub(super) fn init() {
    assert!(ReadyTableBitFields::BITS >= ThreadPriority::BITS);
    unsafe { READY_TABLE.write(SpinLock::new(ReadyTable::default())) };
    let mut w = unsafe { READY_TABLE.assume_init_ref().irqsave_lock() };
    for i in 0..(MAX_THREAD_PRIORITY + 1) as usize {
        w.tables[i].init();
    }
}

#[derive(Debug, Default)]
struct ReadyTable {
    active_tables: ReadyTableBitFields,
    tables: [ArcList<Thread, thread::OffsetOfSchedNode>; (MAX_THREAD_PRIORITY + 1) as usize],
}

impl ReadyTable {
    #[inline]
    fn clear_active_queue(&mut self, bit: u32) -> &mut Self {
        self.active_tables &= !(1 << bit);
        self
    }

    #[inline]
    fn set_active_queue(&mut self, bit: u32) -> &mut Self {
        self.active_tables |= 1 << bit;
        self
    }

    #[inline]
    fn highest_active(&self) -> u32 {
        self.active_tables.trailing_zeros()
    }
}

pub fn next_ready_thread() -> Option<ThreadNode> {
    let mut tbl = unsafe { READY_TABLE.assume_init_ref().irqsave_lock() };
    let highest_active = tbl.highest_active();
    if highest_active > MAX_THREAD_PRIORITY as u32 {
        return None;
    }
    let q = &mut tbl.tables[highest_active as usize];
    let next = q.pop_front();
    assert!(next.is_some());
    if q.is_empty() {
        tbl.clear_active_queue(highest_active);
    }
    assert!(next.as_ref().unwrap().validate_saved_sp());
    return next;
}

// We only queue the thread if old_state equals thread's current state.
pub fn queue_ready_thread(old_state: Uint, t: ThreadNode) -> bool {
    assert!(old_state != thread::READY);
    if !t.transfer_state(old_state, thread::READY) {
        return false;
    }
    assert!(t.validate_saved_sp());
    let mut tbl = unsafe { READY_TABLE.assume_init_ref().irqsave_lock() };
    let priority = t.priority();
    let q = &mut tbl.tables[priority as usize];
    q.push_back(t.clone());
    tbl.set_active_queue(priority as u32);
    return true;
}
