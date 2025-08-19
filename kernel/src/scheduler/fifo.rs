// Copyright (c) 2025 vivo Mobile Communication Co., Ltd.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//       http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

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
    rq.pop_front()
}

pub fn queue_ready_thread(old_state: Uint, t: ThreadNode) -> bool {
    if !t.transfer_state(old_state, thread::READY) {
        return false;
    }
    let mut w = READY_QUEUE.lock();
    let mut rq = LazyCell::get_mut(w.deref_mut()).unwrap();
    rq.push_back(t);
    true
}

pub fn remove_from_ready_thread(mut t: ThreadNode) -> bool {
    todo!()
}
