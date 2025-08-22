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
use super::{SpinLock, SpinLockGuard};
use crate::{
    irq,
    scheduler::{self, InsertMode, OffsetOfWait, WaitEntry, WaitQueue},
    thread::{self, Thread, ThreadNode},
    time::{NO_WAITING, WAITING_FOREVER},
    types::{
        impl_simple_intrusive_adapter, Arc, ArcList, ArcListIterator, AtomicIlistHead as IlistHead,
        Int, ThreadPriority,
    },
};
use alloc::string::String;
use core::{
    cell::{Cell, UnsafeCell},
    sync::atomic::{AtomicU32, Ordering},
};

impl_simple_intrusive_adapter!(OffsetOfMutexNode, Mutex, mutex_node);
type MutexList = ArcList<Mutex, OffsetOfMutexNode>;

#[derive(Debug)]
pub struct Mutex {
    // We let the Spinlock protect the whole Mutex. sort thread with priority
    pending: SpinLock<WaitQueue>,
    nesting_count: AtomicU32,
    owner: UnsafeCell<Option<ThreadNode>>,
    mutex_node: IlistHead<Mutex, OffsetOfMutexNode>,
    wait_time: Cell<usize>,
}

impl Mutex {
    pub const fn new() -> Self {
        Self {
            pending: SpinLock::new(WaitQueue::new()),
            nesting_count: AtomicU32::new(0),
            owner: UnsafeCell::new(None),
            mutex_node: IlistHead::<Mutex, OffsetOfMutexNode>::new(),
            wait_time: Cell::new(0),
        }
    }

    pub fn init(&self) -> bool {
        self.pending.irqsave_lock().init()
    }

    #[inline]
    fn nesting_count(&self) -> u32 {
        self.nesting_count.load(Ordering::Relaxed)
    }

    #[inline]
    fn increment_nesting_count(&self) -> u32 {
        self.nesting_count.fetch_add(1, Ordering::Relaxed)
    }

    #[inline]
    fn decrement_nesting_count(&self) -> u32 {
        self.nesting_count.fetch_sub(1, Ordering::Relaxed)
    }

    #[inline]
    fn set_owner(&self, t: Option<ThreadNode>) {
        unsafe {
            *self.owner.get() = t;
        }
    }

    #[inline]
    fn get_owner(&self) -> Option<ThreadNode> {
        unsafe { (*self.owner.get()).clone() }
    }

    pub fn create() -> Arc<Self> {
        let mutex = Arc::new(Self::new());
        mutex.init();
        mutex
    }

    pub fn pend_for(&self, timeout: usize) -> bool {
        assert!(!irq::is_in_irq());

        let t = scheduler::current_thread();
        let mut w = self.pending.irqsave_lock();
        let mutex = unsafe { MutexList::make_arc_from(&self.mutex_node) };

        if self.nesting_count() == 0 {
            self.increment_nesting_count();
            self.set_owner(Some(t.clone()));

            let mut mlist = &mut t.lock().mutex_list;
            mlist.push_back(mutex.clone());
            return true;
        }

        assert!(self.get_owner().is_some());
        let owner = self.get_owner().unwrap();
        if Thread::id(&owner) == Thread::id(&t) {
            self.increment_nesting_count();
            return true;
        }

        if timeout == NO_WAITING {
            return false;
        }

        mutex.wait_time.set(timeout);
        mutex_pend(mutex, w, timeout)
    }

    pub fn post(&self) {
        assert!(!irq::is_in_irq());

        let t = scheduler::current_thread();
        if self.get_owner().is_none() {
            assert_eq!(self.nesting_count(), 0);
            panic!("the mutex is free, cannot be released");
        }

        let mut w = self.pending.irqsave_lock();
        let owner = self.get_owner().unwrap();
        if Thread::id(&owner) != Thread::id(&t) {
            panic!("mutex only can be released by owner");
        }

        if self.decrement_nesting_count() > 1 {
            return;
        }

        let mut pending_flag = false;
        let mut mutex = unsafe { MutexList::make_arc_from(&self.mutex_node) };
        while let Some(next) = w.pop_front() {
            let t = next.thread.clone();
            if mutex.wait_time.get() != WAITING_FOREVER {
                if let Some(timer) = &t.timer {
                    if !timer.is_activated() {
                        continue;
                    }
                    timer.stop();
                }
            }
            pending_flag = true;
            MutexList::detach(&mut mutex);
            self.set_owner(Some(t.clone()));
            self.increment_nesting_count();

            let ok = scheduler::queue_ready_thread(thread::SUSPENDED, t.clone());
            debug_assert!(ok);
            let mut mlist = &mut t.lock().mutex_list;
            mlist.push_back(mutex.clone());
            restore_priority();
            break;
        }
        if !pending_flag {
            self.set_owner(None);
            MutexList::detach(&mut mutex);
            if !restore_priority() {
                return;
            }
        }

        drop(w);
        scheduler::yield_me_now_or_later();
    }
}

impl Default for Mutex {
    fn default() -> Self {
        Mutex::new()
    }
}

impl !Send for Mutex {}
unsafe impl Sync for Mutex {}

fn delete_from_wait_queue(mut w: &mut SpinLockGuard<'_, WaitQueue>, owner: ThreadNode) {
    for mut entry in w.iter() {
        if Thread::id(&entry.thread) == Thread::id(&owner) {
            WaitQueue::detach(&mut entry);
        }
    }
}

fn resort_mutex_list(mutex: Arc<Mutex>, owner: ThreadNode) {
    let mut w = mutex.pending.irqsave_lock();
    delete_from_wait_queue(&mut w, owner.clone());
    scheduler::insert_wait_queue(&mut w, owner, InsertMode::InsertByPrio);
}

fn mutex_adapt_priority(mutex: Arc<Mutex>, priority: ThreadPriority) {
    let mut owner = mutex.get_owner().unwrap();
    let mut t = owner.lock();

    while t.priority() > priority {
        if t.state() == thread::READY {
            scheduler::remove_from_ready_thread(owner.clone());
            // suspend momentarily
            unsafe { owner.set_state(thread::SUSPENDED) };
            t.set_priority(priority);
            scheduler::queue_ready_thread(thread::SUSPENDED, owner.clone());
            return;
        }
        t.set_priority(priority);
        if t.state() != thread::SUSPENDED || t.pend_mutex.is_none() {
            return;
        }

        let mutex = owner.pend_mutex.as_ref().unwrap();
        resort_mutex_list(mutex.clone(), owner.clone());

        drop(t);
        owner = mutex.get_owner().unwrap();
        t = owner.lock();
    }
}

fn mutex_pend(mutex: Arc<Mutex>, mut w: SpinLockGuard<'_, WaitQueue>, timeout: usize) -> bool {
    let current = crate::scheduler::current_thread();
    let priority = current.priority();
    current.lock().pend_mutex = Some(mutex.clone());

    mutex_adapt_priority(mutex.clone(), priority);
    let out_time = scheduler::suspend_me_with_timeout(w, timeout, InsertMode::InsertByPrio);
    if out_time {
        return false;
    }
    true
}

fn mutex_pend_task_priority() -> ThreadPriority {
    let current = crate::scheduler::current_thread();
    let mut priority = current.get_origin_priority();

    let mlist = &current.lock().mutex_list;
    for entry in mlist.iter() {
        let mut w = entry.pending.irqsave_lock();
        if let Some(pending_thread) = w.front() {
            if pending_thread.thread.priority() < priority {
                priority = pending_thread.thread.priority();
            }
        }
        drop(w)
    }
    priority
}

fn restore_priority() -> bool {
    let current = crate::scheduler::current_thread();
    if current.priority() == current.get_origin_priority() {
        return false;
    }

    let max_priority = mutex_pend_task_priority();
    if max_priority == current.priority() {
        return false;
    }

    let mut curr = current.lock();
    curr.set_priority(max_priority);
    true
}

#[cfg(cortex_m)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::ConstBarrier;
    use blueos_test_macro::{only_test, test};

    #[test]
    fn test_mutex_new() {
        // Test successful creation with valid counter
        let mutex = Mutex::new();
        assert_eq!(mutex.nesting_count(), 0);

        let mutex1 = Mutex::new();
        assert_eq!(mutex1.nesting_count(), 0);
    }

    #[test]
    fn test_mutex_init() {
        let mutex = Mutex::new();

        // Test initialization
        let result = mutex.init();
        assert!(result);

        // Test multiple initializations
        let result2 = mutex.init();
        assert!(!result2);
    }

    #[test]
    fn test_mutex_pend_post_success() {
        let mutex = Mutex::create();

        // Test successful pend
        let result = mutex.pend_for(WAITING_FOREVER);
        assert!(result);
        assert_eq!(mutex.nesting_count(), 1);

        // post operations
        mutex.post();
        assert_eq!(mutex.nesting_count(), 0);

        // pend
        let result = mutex.pend_for(WAITING_FOREVER);
        assert!(result);
        assert_eq!(mutex.nesting_count(), 1);

        // post operations
        mutex.post();
        assert_eq!(mutex.nesting_count(), 0);
    }

    #[test]
    fn test_mutex_multi_pend_success() {
        let mutex = Mutex::create();

        // Test successful pend
        let result = mutex.pend_for(WAITING_FOREVER);
        assert!(result);
        assert_eq!(mutex.nesting_count(), 1);

        // pend operations
        assert!(mutex.pend_for(WAITING_FOREVER));
        assert_eq!(mutex.nesting_count(), 2);
    }

    // #[test]
    // #[should_panic(expected = "mutex only can be released by owner")]
    // fn test_mutex_multi_post_panic() {
    //     let mutex = Mutex::new();
    //     mutex.init();

    //     // Test successful pend
    //     let result = mutex.pend_for(10);
    //     assert!(result);
    //     assert_eq!(mutex.nesting_count(), 1);

    //     mutex.post();
    //     assert_eq!(mutex.nesting_count(), 0);

    //     mutex.post();
    // }

    #[test]
    fn test_mutex_multi_pend_post_success() {
        let mutex = Mutex::create();

        // Test 10x pend operations
        for i in 0..10 {
            let result = mutex.pend_for(10);
            assert!(result);
            assert_eq!(mutex.nesting_count(), i + 1);
        }

        // test 10x post operations
        for i in 0..10 {
            assert_eq!(mutex.nesting_count(), 10 - i);
            mutex.post();
        }
        assert_eq!(mutex.nesting_count(), 0);

        // Test 10x pend operations again
        for i in 0..10 {
            let result = mutex.pend_for(10);
            assert!(result);
            assert_eq!(mutex.nesting_count(), i + 1);
        }

        // test 10x post operations
        for i in 0..10 {
            assert_eq!(mutex.nesting_count(), 10 - i);
            mutex.post();
        }
        assert_eq!(mutex.nesting_count(), 0);
    }

    // #[test]
    // #[should_panic(expected = "mutex only can be released by owner")]
    // fn test_mutex_multi_thread() {
    //     let mutex = Mutex::create();
    //     let mutex_consumer = mutex.clone();

    //     let consumer = thread::spawn(move || {
    //         println!("consumer is posting mutex");
    //         mutex_consumer.post();

    //     });
    //     mutex.pend_for(10);
    //     println!("host is pending mutex");
    //     scheduler::yield_me();
    // }

    #[test]
    fn test_mutex_multi_thread1() {
        let mutex = Mutex::create();

        let mutex_consumer = mutex.clone();

        let consumer = thread::spawn(move || {
            assert_eq!(mutex_consumer.nesting_count(), 1);
            mutex_consumer.pend_for(10);
            assert_eq!(mutex_consumer.nesting_count(), 1);
        });
        mutex.pend_for(10);
        assert_eq!(mutex.nesting_count(), 1);
        scheduler::yield_me();
        mutex.post();
    }

    #[test]
    fn test_mutex_multi_thread_nowaiting() {
        let mutex = Mutex::create();

        let mutex_consumer = mutex.clone();

        let consumer = thread::spawn(move || {
            assert_eq!(mutex_consumer.nesting_count(), 1);
            let result = mutex_consumer.pend_for(NO_WAITING);
            assert!(!result);
        });
        mutex.pend_for(10);
        assert_eq!(mutex.nesting_count(), 1);
        scheduler::yield_me();
        mutex.post();
    }

    use crate::time;
    #[test]
    fn test_mutex_multi_thread_timeout() {
        let mutex = Mutex::create();

        let mutex_consumer = mutex.clone();

        let consumer = thread::spawn(move || {
            assert_eq!(mutex_consumer.nesting_count(), 1);
            let result = mutex_consumer.pend_for(5);
            assert!(!result);
        });
        mutex.pend_for(10);
        assert_eq!(mutex.nesting_count(), 1);

        scheduler::yield_me();
        let start = time::get_sys_ticks();
        let mut current = time::get_sys_ticks();
        while current - start < 5 {
            if current != time::get_sys_ticks() {
                current = time::get_sys_ticks();
            }
        }
        mutex.post();
    }

    #[test]
    fn test_mutex_multi_thread_priority1() {
        let sync0 = Arc::new(ConstBarrier::<{ 2 }>::new());
        let consumer_sync0 = sync0.clone();
        let sync1 = Arc::new(ConstBarrier::<{ 2 }>::new());
        let consumer_sync1 = sync1.clone();
        let mutex = Mutex::create();
        let mutex_consumer = mutex.clone();
        let current = crate::scheduler::current_thread();
        let origin_priority = current.priority();
        let consumer = thread::spawn(move || {
            // We have to wait the outer thread to change this thread's priority.
            consumer_sync0.wait();
            let current = crate::scheduler::current_thread();
            assert_eq!(origin_priority - 1, current.priority());
            // We have to wait the outer thread to get the mutex first.
            consumer_sync1.wait();
            // Now the outer thread has got the mutex, the following pend_for
            // will increase the priority of the outer thread temporarily.
            let result = mutex_consumer.pend_for(10);
            assert!(result);
        });
        let current = crate::scheduler::current_thread();
        let thread = consumer.unwrap().clone();
        let mut w = thread.lock();
        w.set_priority(origin_priority - 1);
        drop(w);
        sync0.wait();
        mutex.pend_for(10);
        assert_eq!(mutex.nesting_count(), 1);
        sync1.wait();
        scheduler::yield_me();
        assert_eq!(origin_priority - 1, current.priority());
        mutex.post();
        assert_eq!(origin_priority, current.priority());
    }
}
