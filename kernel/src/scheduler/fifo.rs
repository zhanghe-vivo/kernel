extern crate alloc;
use crate::{support, thread, thread::ThreadNode, types::Uint};
use alloc::collections::LinkedList;
use core::{cell::LazyCell, ops::DerefMut};
use spin::Mutex;

static READY_QUEUE: Mutex<LazyCell<LinkedList<ThreadNode>>> =
    Mutex::new(LazyCell::new(|| LinkedList::new()));

pub(super) fn init() {
    let mut w = READY_QUEUE.lock();
    LazyCell::force_mut(w.deref_mut());
}

pub fn next_ready_thread() -> Option<ThreadNode> {
    let mut w = READY_QUEUE.lock();
    let mut rq = LazyCell::get_mut(w.deref_mut()).unwrap();
    return rq.pop_front();
}

pub fn queue_ready_thread(old_state: Uint, t: ThreadNode) -> bool {
    if !t.transfer_state(old_state, thread::READY) {
        return false;
    }
    let mut w = READY_QUEUE.lock();
    let mut rq = LazyCell::get_mut(w.deref_mut()).unwrap();
    rq.push_back(t);
    return true;
}
