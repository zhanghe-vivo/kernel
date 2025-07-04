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

// Asynk contains a simple executor, however runs fast.

extern crate alloc;
use crate::{
    config::MAX_THREAD_PRIORITY,
    scheduler, static_arc,
    support::ArcBufferingQueue,
    sync::{atomic_wait, ISpinLock, SpinLockGuard},
    thread::{self, Entry, SystemThreadStorage, ThreadKind, ThreadNode},
    types::{impl_simple_intrusive_adapter, Arc, IlistHead},
};
use alloc::boxed::Box;
use core::{
    future::Future,
    mem::MaybeUninit,
    pin::Pin,
    sync::atomic::{AtomicUsize, Ordering},
    task::{Context, Poll, Waker},
};

impl_simple_intrusive_adapter!(TaskletNode, Tasklet, node);
impl_simple_intrusive_adapter!(TaskletLock, Tasklet, lock);

pub struct Tasklet {
    pub node: IlistHead<Tasklet, TaskletNode>,
    lock: ISpinLock<Tasklet, TaskletLock>,
    future: Pin<Box<dyn Future<Output = ()>>>,
    blocked: Option<ThreadNode>,
}

impl Tasklet {
    pub fn new(future: Pin<Box<dyn Future<Output = ()>>>) -> Self {
        Self {
            node: IlistHead::new(),
            future,
            lock: ISpinLock::new(),
            blocked: None,
        }
    }

    pub fn lock(&self) -> SpinLockGuard<'_, Tasklet> {
        self.lock.irqsave_lock()
    }
}

type AsyncWorkQueue = ArcBufferingQueue<Tasklet, TaskletNode, 2>;
static POLLER_STORAGE: SystemThreadStorage = SystemThreadStorage::new(ThreadKind::AsyncPoller);
static mut POLLER: MaybeUninit<ThreadNode> = MaybeUninit::zeroed();
static POLLER_WAKER: AtomicUsize = AtomicUsize::new(0);
static_arc! {
    ASYNC_WORK_QUEUE(AsyncWorkQueue, AsyncWorkQueue::new()),
}

pub(crate) fn init() {
    ASYNC_WORK_QUEUE.init_queues();
    let poller = thread::build_static_thread(
        unsafe { &mut POLLER },
        &POLLER_STORAGE,
        MAX_THREAD_PRIORITY,
        thread::CREATED,
        Entry::C(poll),
        ThreadKind::AsyncPoller,
    );
    let ok = scheduler::queue_ready_thread(thread::CREATED, poller);
    debug_assert!(ok);
}

fn create_tasklet(future: impl Future<Output = ()> + 'static) -> Arc<Tasklet> {
    let future = Box::pin(future);
    let mut task = Arc::new(Tasklet::new(future));
    return task;
}

pub fn block_on(future: impl Future<Output = ()> + 'static) {
    let t = scheduler::current_thread();
    let mut task = create_tasklet(future);
    task.lock().blocked = Some(t.clone());
    scheduler::suspend_me_with_hook(move || {
        let ok = t.transfer_state(thread::RUNNING, thread::SUSPENDED);
        assert!(ok);
        enqueue_active_tasklet(task);
        wake_poller();
    });
}

fn wake_poller() {
    POLLER_WAKER.fetch_add(1, Ordering::Release);
    atomic_wait::atomic_wake(&POLLER_WAKER as *const _ as usize, 1);
}

pub fn spawn(future: impl Future<Output = ()> + 'static) -> Arc<Tasklet> {
    let task = create_tasklet(future);
    enqueue_active_tasklet(task.clone());
    wake_poller();
    return task;
}

pub fn enqueue_active_tasklet(t: Arc<Tasklet>) {
    let mut q = ASYNC_WORK_QUEUE.get_active_queue();
    let _ = t.lock();
    q.push_back(t.clone());
}

fn poll_inner() {
    let mut ctx = Context::from_waker(Waker::noop());
    let mut w = ASYNC_WORK_QUEUE.advance_active_queue();
    for mut task in w.iter() {
        let mut l = task.lock();
        match l.future.as_mut().poll(&mut ctx) {
            Poll::Ready(()) => {
                if let Some(t) = l.blocked.take() {
                    scheduler::queue_ready_thread(thread::SUSPENDED, t);
                }
                // If we detach the task what ever it's ready or
                // pending, it would be edge-level triggered. Now
                // we're using level-trigger mode conservatively.
                AsyncWorkQueue::WorkList::detach(&mut task.clone());
            }
            // FIXME: This is not an efficient impl right now. We
            // might need a waker for each future, so that the poller
            // doesn't need to poll all futures when woken up.
            _ => {}
        }
    }
}

extern "C" fn poll() {
    loop {
        poll_inner();
        let n = POLLER_WAKER.load(Ordering::Acquire);
        atomic_wait::atomic_wait(&POLLER_WAKER as *const _ as usize, n, None);
    }
}
