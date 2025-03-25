/// A simple futex implementation featured WAIT & WAKE. Most code is from redox.
// MIT License
// Copyright (c) 2017 Jeremy Soller
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.
use crate::{cpu::Cpu, thread::Thread};
use alloc::{boxed::Box, collections::VecDeque};
use bluekernel_infra::{
    klibc::{EAGAIN, EBUSY, EINVAL, ESRCH},
    list::doubly_linked_list::LinkedListNode,
};
use core::{
    marker::PhantomPinned,
    pin::Pin,
    sync::atomic::{AtomicUsize, Ordering},
};
use pinned_init::{pin_data, pin_init, InPlaceInit, PinInit};
use spin::RwLock;

// Currently, the kernel is using C-style intrusive doubly linked list to contain objects.
// This design naturally makes element belonging to only one list at any time if only one ListHead
// embbeded in a struct.
// ListHead does not have value semantics, but simply a marker. The memory to contain ListHead's
// bits must remain unchanged during ListHead's whole lifetime. It's called pinned, or !Unpin from Rust's view.
// Since the field of ListHead is pinned, the whole struct is pinned.
// FutexEntry must use ListHead to contain waiting threads to be consistent with current kernel's design.
// To make FutexEntry working with common Rust types, use a Pin<Box<T>> to hold FutexEntry,
// overcome the limitation of !Unpin.
type FutexList = VecDeque<Pin<Box<FutexEntry>>>;

static FUTEXES: RwLock<FutexList> = RwLock::new(FutexList::new());

// TODO: Use hash table to map addr to waiting_threads.
#[pin_data]
pub(crate) struct FutexEntry {
    addr: usize,
    #[pin]
    waiting_threads: LinkedListNode,
    #[pin]
    _pin: PhantomPinned,
}

impl FutexEntry {
    // PinInit serves similar to C++ ctor's member initializer list, not ctor's body.
    fn new(addr: usize) -> impl PinInit<Self> {
        pin_init!(Self { addr: addr, waiting_threads <- LinkedListNode::new(), _pin: PhantomPinned })
    }
}

// TODO: Support timeout.
pub(crate) fn atomic_wait(addr: usize, val: usize) -> Result<(), i32> {
    let ptr: *const AtomicUsize = addr as *const AtomicUsize;
    let fetched = unsafe { &*ptr }.load(Ordering::Acquire);
    if fetched != val {
        // TODO: We should use the thread_local ERRNO
        return Err(EAGAIN);
    }
    let scheduler = Cpu::get_current_scheduler();
    let Some(current_thread) = scheduler.get_current_thread() else {
        return Err(ESRCH);
    };
    let current_thread_ptr: *mut Thread = current_thread.as_ptr();
    let Ok(mut pinned_box) = Box::pin_init(FutexEntry::new(addr)) else {
        return Err(EINVAL);
    };
    if scheduler.is_sched_locked() {
        return Err(EBUSY);
    }
    scheduler.preempt_disable();
    if unsafe { &mut *current_thread_ptr }.suspend(crate::thread::SuspendFlag::Uninterruptible) {
        {
            let boxed = pinned_box.as_mut();
            let waiting_threads =
                unsafe { Pin::new_unchecked(&mut boxed.get_unchecked_mut().waiting_threads) };
            let mut current_thread_node =
                unsafe { Pin::new_unchecked(&mut (*current_thread_ptr).list_node) };
            assert!(current_thread_node.is_empty());
            current_thread_node.as_mut().insert_after(waiting_threads);
        }
        let mut futexes = FUTEXES.write();
        futexes.push_back(pinned_box);
        drop(futexes);
        scheduler.do_task_schedule();
        scheduler.preempt_enable();
        Ok(())
    } else {
        scheduler.preempt_enable();
        return Err(EAGAIN);
    }
}

pub(crate) fn atomic_wake(addr: usize, how_many: usize) -> Result<usize, ()> {
    assert!((addr as *const u8).is_aligned_to(core::mem::size_of::<usize>()));
    let mut woken = 0;
    let mut i = 0;
    let mut futexes = FUTEXES.write();
    let mut resched = false;
    while i < futexes.len() && woken < how_many {
        if futexes[i].addr == addr {
            let waiting_threads =
                unsafe { &mut (futexes[i].as_mut().get_unchecked_mut().waiting_threads) };
            let mut loop_count = 0usize;
            while let Some(elem) = waiting_threads.next() {
                loop_count += 1;
                let thread_ptr = unsafe { crate::thread_list_node_entry!(elem.as_ptr()) };
                // We'll let resume() remove the elem, since the removal is performed in critical region,
                // thus protected.
                // FIXME: What if resume() doesn't put the thread in scheduler's queue.
                resched |= unsafe { (&mut *thread_ptr).resume() };
                woken += 1;
                if woken >= how_many {
                    break;
                }
            }
            if waiting_threads.is_empty() {
                futexes.swap_remove_back(i);
            }
        }
        i += 1
    }
    drop(futexes);
    if resched {
        Cpu::get_current_scheduler().do_task_schedule();
    }
    Ok(woken)
}
