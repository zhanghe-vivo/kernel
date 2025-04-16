use alloc::sync::Arc;
use bluekernel::{
    clock,
    sync::futex::*,
    thread::{Thread, ThreadBuilder},
};
use bluekernel_test_macro::test;
use core::sync::atomic::AtomicUsize;
use libc::ETIMEDOUT;

#[test]
fn test_futex_timeout() {
    // Create a stack variable to use as futex address
    let futex_addr = AtomicUsize::new(0);
    let val = 0;
    let timeout = clock::tick_from_millisecond(1000);

    // Use the address of the atomic variable
    let addr = &futex_addr as *const AtomicUsize as usize;
    let res = atomic_wait(addr, val, timeout);

    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), ETIMEDOUT);
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

#[test]
fn test_futex_thread_wait() {
    let futex = Arc::new(AtomicUsize::new(0));
    let futex_clone = futex.clone();

    // Thread entry function
    extern "C" fn thread_entry(arg: *mut core::ffi::c_void) {
        let futex = unsafe { Arc::from_raw(arg as *const AtomicUsize) };
        let addr = &*futex as *const AtomicUsize as usize;
        let timeout = clock::tick_from_millisecond(2000);

        // Wait for futex signal
        let res = atomic_wait(addr, 0, timeout);
        assert!(res.is_ok());
    }

    // Create new thread using ThreadBuilder
    let thread = ThreadBuilder::default()
        .name(unsafe { core::ffi::CStr::from_bytes_with_nul_unchecked(b"futex_test\0") })
        .entry_fn(thread_entry)
        .arg(Arc::into_raw(futex_clone) as *mut core::ffi::c_void)
        .stack_size(4096) // 4KB stack
        .priority(1)
        .tick(50)
        .build_from_heap()
        .expect("Failed to create thread");

    unsafe { (&mut *thread.as_ptr()).start() };

    // Sleep a bit to ensure thread starts
    let _ = Thread::msleep(1000);

    // Wake up the waiting thread
    let addr = &*futex as *const AtomicUsize as usize;
    let res = atomic_wake(addr, 1);
    assert!(res.is_ok());
    assert_eq!(res.unwrap(), 1);
}
