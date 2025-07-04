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

use blueos::{
    scheduler,
    sync::{atomic_wait, atomic_wake, Semaphore},
    thread::{Builder, Entry, Thread},
    types::Arc,
};
use blueos_test_macro::test;
use core::sync::atomic::{AtomicUsize, Ordering};
use libc::ETIMEDOUT;

#[test]
fn test_futex_timeout() {
    // Create a stack variable to use as futex address
    //     let futex_addr = AtomicUsize::new(0);
    //     let val = 0;
    //     let timeout = Some(1000);
    //
    // Use the address of the atomic variable
    //    let addr = &futex_addr as *const AtomicUsize as usize;
    //    let res = atomic_wait(addr, val, timeout);
    //
    //    assert!(res.is_err());
    //    assert_eq!(res.unwrap_err(), ETIMEDOUT);
}

#[test]
fn test_futex_wake() {
    // Create a stack variable to use as futex address
    let futex_addr = AtomicUsize::new(0);

    // Use the address of the atomic variable
    let addr = &futex_addr as *const AtomicUsize as usize;

    // Wake should succeed even if no one is waiting
    let res = atomic_wake(addr, 1);
    assert!(res.is_ok());
    assert_eq!(res.unwrap(), 0); // No threads were woken up
}

static TEST_FUTEX_WAIT: AtomicUsize = AtomicUsize::new(0);

#[test]
fn test_futex_thread_wait() {
    // Thread entry function
    extern "C" fn thread_entry(arg: *mut core::ffi::c_void) {
        let addr = &TEST_FUTEX_WAIT as *const _ as usize;
        // Wait for futex signal.
        match atomic_wait(addr, 0, None) {
            Ok(_) => {
                TEST_FUTEX_WAIT.store(42, Ordering::Relaxed);
            }
            Err(_) => {
                assert_eq!(TEST_FUTEX_WAIT.load(Ordering::Relaxed), 1);
            }
        }
    }
    let t = Builder::new(Entry::Posix(thread_entry, core::ptr::null_mut())).build();
    scheduler::queue_ready_thread(t.state(), t);
    #[cfg(aarch64)]
    scheduler::yield_me();

    TEST_FUTEX_WAIT.fetch_add(1, Ordering::Relaxed);
    // Wake up the waiting thread
    let addr = &TEST_FUTEX_WAIT as *const _ as usize;
    match atomic_wake(addr, 1) {
        Ok(c) => {
            // FIXME: aarch64 not suport timeout yet
            #[cfg(aarch64)]
            scheduler::yield_me();
            if c == 1 {
                loop {
                    let val = TEST_FUTEX_WAIT.load(Ordering::Relaxed);
                    if val == 42 {
                        break;
                    }
                    core::hint::spin_loop();
                }
            } else {
            }
        }
        _ => panic!("Unable to wake a thread"),
    }
}
