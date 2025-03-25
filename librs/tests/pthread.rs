extern crate alloc;
use alloc::vec::Vec;
use core::{
    cell::{Cell, RefCell},
    ffi::{c_int, c_size_t, c_void},
    intrinsics::transmute,
    mem::{align_of, size_of, MaybeUninit},
    sync::atomic::{AtomicI8, AtomicUsize, Ordering},
};
use libc::{
    clockid_t, pthread_attr_t, pthread_cond_t, pthread_condattr_t, pthread_key_t, pthread_mutex_t,
    pthread_mutexattr_t, pthread_t, EDEADLK, EINVAL, ESRCH,
};
use librs::{
    free, posix_memalign,
    pthread::*,
    sync::{
        cond::{Cond, CondAttr},
        mutex::{Mutex, MutexAttr},
        waitval::Waitval,
    },
};

extern "C" fn mutex_lock_unlock(arg: *mut c_void) -> *mut c_void {
    let mutex = arg.cast::<pthread_mutex_t>();
    assert_eq!(pthread_mutex_lock(mutex), 0);
    assert_eq!(pthread_mutex_unlock(mutex), 0);
    core::ptr::null_mut()
}

#[test_case]
fn test_single_thread_mutex() {
    let mut mutex: pthread_mutex_t = unsafe { MaybeUninit::zeroed().assume_init() };
    pthread_mutex_init(&mut mutex as *mut _, core::ptr::null());
    unsafe {
        mutex_lock_unlock(transmute::<*mut pthread_mutex_t, *mut c_void>(
            &mut mutex as *mut pthread_mutex_t,
        ));
    }
}

#[test_case]
fn test_multi_thread_mutex() {
    let mut mutex: pthread_mutex_t = unsafe { MaybeUninit::zeroed().assume_init() };
    let num_threads = 4;
    let mut threads = Vec::new();
    for _ in 0..num_threads {
        let mut t: pthread_t = 0;
        let rc = pthread_create(
            &mut t as *mut pthread_t,
            core::ptr::null_mut(),
            mutex_lock_unlock,
            &mut mutex as *mut pthread_mutex_t as *mut c_void,
        );
        assert_eq!(rc, 0);
        threads.push(t);
    }
    for t in threads {
        pthread_join(t, core::ptr::null_mut());
    }
}

struct Waiter(*mut pthread_cond_t, *mut pthread_mutex_t, bool);

extern "C" fn cond_wait(arg: *mut c_void) -> *mut c_void {
    let waiter = unsafe { &*arg.cast::<Waiter>() };
    assert_eq!(pthread_mutex_lock(waiter.1), 0);
    while !waiter.2 {
        assert_eq!(pthread_cond_wait(waiter.0, waiter.1), 0);
    }
    assert_eq!(pthread_mutex_unlock(waiter.1), 0);
    core::ptr::null_mut()
}

#[test_case]
fn test_mult_thread_cond() {
    let mut cond: pthread_cond_t = unsafe { MaybeUninit::zeroed().assume_init() };
    let condattr: CondAttr = CondAttr::default();
    pthread_cond_init(
        &mut cond as *mut pthread_cond_t,
        &condattr as *const CondAttr as *const pthread_condattr_t,
    );
    let mut mutex: pthread_mutex_t = unsafe { MaybeUninit::zeroed().assume_init() };
    let mut waiter = Waiter(
        &mut cond as *mut pthread_cond_t,
        &mut mutex as *mut pthread_mutex_t,
        false,
    );
    let mut threads = Vec::new();
    let num_threads = 4;
    for _ in 0..num_threads {
        let mut t: pthread_t = 0;
        let rc = pthread_create(
            &mut t as *mut pthread_t,
            core::ptr::null_mut(),
            cond_wait,
            &mut waiter as *mut Waiter as *mut c_void,
        );
        assert_eq!(rc, 0);
        threads.push(t);
    }
    assert_eq!(pthread_mutex_lock(waiter.1), 0);
    waiter.2 = true;
    assert_eq!(pthread_cond_signal(waiter.0), 0);
    assert_eq!(pthread_mutex_unlock(waiter.1), 0);
    for t in threads {
        pthread_join(t, core::ptr::null_mut());
    }
}

#[thread_local]
static THREAD_LOCAL_CHECK: Cell<usize> = Cell::new(42);

#[thread_local]
static LOCAL_VEC: RefCell<Vec<i32>> = RefCell::new(Vec::new());

#[test_case]
fn test_complex_thread_local() {
    fn is_prime(n: i32) -> bool {
        let mut i = 2;
        while i * i <= n {
            if n % i == 0 {
                return false;
            }
            i += 1;
        }
        true
    }

    for i in 2..1024 {
        if is_prime(i) {
            LOCAL_VEC.borrow_mut().push(i);
        }
    }
}

extern "C" fn increase_counter(arg: *mut c_void) -> *mut c_void {
    assert_eq!(THREAD_LOCAL_CHECK.get(), 42);
    let counter: *mut AtomicUsize = unsafe { transmute(arg) };
    THREAD_LOCAL_CHECK.set(unsafe { &*counter }.fetch_add(1, Ordering::Release) + 1);
    core::ptr::null_mut()
}

#[test_case]
fn test_pthread_create_and_join() {
    let num_threads = 4;
    let mut threads = Vec::new();
    let mut counter = AtomicUsize::new(0);
    for _ in 0..num_threads {
        let mut t: pthread_t = 0;
        let arg: *mut c_void =
            unsafe { transmute(&mut counter as *mut AtomicUsize as *mut c_void) };
        let rc = pthread_create(
            &mut t as *mut pthread_t,
            core::ptr::null(),
            increase_counter,
            arg,
        );
        assert_eq!(rc, 0);
        threads.push(t);
    }
    let mut num_joined = 0;
    for t in threads {
        assert_eq!(pthread_join(t, core::ptr::null_mut()), 0);
        num_joined += 1;
    }
    assert_eq!(num_threads, num_joined);
    assert_eq!(counter.load(Ordering::Acquire), num_threads);
}

#[test_case]
fn test_pthread_create_and_detach() {
    let num_threads = 4;
    let mut threads = Vec::new();
    let mut counter = AtomicUsize::new(0);
    let mut num_detached = 0;
    for _ in 0..num_threads {
        let mut t: pthread_t = 0;
        let arg: *mut c_void =
            unsafe { transmute(&mut counter as *mut AtomicUsize as *mut c_void) };
        let rc = pthread_create(
            &mut t as *mut pthread_t,
            core::ptr::null(),
            increase_counter,
            arg,
        );
        assert_eq!(rc, 0);
        if pthread_detach(t) == 0 {
            num_detached += 1;
            continue;
        }
        threads.push(t);
    }
    let mut num_joined = 0;
    for t in threads {
        assert_eq!(pthread_join(t, core::ptr::null_mut()), 0);
        num_joined += 1;
    }
    assert_eq!(num_joined + num_detached, num_threads);
}

#[test_case]
fn test_thread_local() {
    assert_eq!(THREAD_LOCAL_CHECK.get(), 42);
}
