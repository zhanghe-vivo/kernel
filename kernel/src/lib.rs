// TOTAL-TIMEOUT: 8
// ASSERT-SUCC: Done kernel unittests
// ASSERT-FAIL: Oops:

#![no_std]
#![allow(internal_features)]
#![allow(unused)]
#![allow(dead_code)]
#![feature(alloc_error_handler)]
#![feature(alloc_layout_extra)]
#![feature(allocator_api)]
#![feature(box_as_ptr)]
#![feature(c_size_t)]
#![feature(c_variadic)]
#![feature(core_intrinsics)]
#![feature(coverage_attribute)]
#![feature(fn_align)]
#![feature(inherent_associated_types)]
#![feature(lazy_get)]
#![feature(link_llvm_intrinsics)]
#![feature(linkage)]
#![feature(macro_metavar_expr)]
#![feature(map_try_insert)]
#![feature(naked_functions)]
#![feature(negative_impls)]
#![feature(new_zeroed_alloc)]
#![feature(noop_waker)]
#![feature(pointer_is_aligned_to)]
#![feature(trait_upcasting)]
// Attributes applied when we're testing the kernel.
#![cfg_attr(test, no_main)]
#![cfg_attr(test, feature(custom_test_frameworks))]
#![cfg_attr(test, test_runner(tests::kernel_unittest_runner))]
#![cfg_attr(test, reexport_test_harness_main = "run_kernel_unittests")]

extern crate alloc;

pub mod ffi {
    #[no_mangle]
    pub extern "C" fn disable_local_irq_save() -> usize {
        crate::arch::disable_local_irq_save()
    }

    #[no_mangle]
    pub extern "C" fn enable_local_irq_restore(val: usize) {
        crate::arch::enable_local_irq_restore(val)
    }
}

pub mod allocator;
pub(crate) mod arch;
pub mod asynk;
pub(crate) mod boards;
pub(crate) mod boot;
pub(crate) mod config;
pub(crate) mod console;
pub(crate) mod devices;
pub mod error;
pub(crate) mod irq;
pub(crate) mod logger;
pub mod scheduler;
pub mod support;
pub mod sync;
pub mod syscall_handlers;
pub mod thread;
pub(crate) mod time;
pub mod types;
pub mod vfs;

pub use syscall_handlers as syscalls;

#[macro_export]
macro_rules! debug {
    ($($tt:tt)*) => {{}};
}

pub(crate) static TRACER: spin::Mutex<()> = spin::Mutex::new(());

#[macro_export]
macro_rules! trace {
    ($($tt:tt)*) => {{
        let dig = $crate::support::DisableInterruptGuard::new();
        let l = $crate::TRACER.lock();
        #[cfg(target_pointer_width="32")]
        semihosting::eprint!("[C:{:02} SP:0x{:08x}] ",
                             $crate::arch::current_cpu_id(),
                             $crate::arch::current_sp());
        #[cfg(target_pointer_width="64")]
        semihosting::eprint!("[C:{:02} SP:0x{:016x}] ",
                             $crate::arch::current_cpu_id(),
                             $crate::arch::current_sp());
        semihosting::eprintln!($($tt)*);
        drop(l);
        drop(dig);
    }};
}

#[cfg(test)]
mod tests {
    extern crate alloc;
    use super::*;
    use crate::{
        allocator, allocator::KernelAllocator, config, support::DisableInterruptGuard, sync,
    };
    use bluekernel_header::syscalls::NR::Nop;
    use bluekernel_kconfig::NUM_CORES;
    use bluekernel_test_macro::test;
    use core::{
        mem::MaybeUninit,
        panic::PanicInfo,
        sync::atomic::{AtomicUsize, Ordering},
    };
    use spin::Mutex;
    use thread::{Entry, SystemThreadStorage, Thread, ThreadKind, ThreadNode};

    #[used]
    #[link_section = ".bk_app_array"]
    static INIT_TEST: extern "C" fn() = init_test;

    extern "C" fn test_main() {
        run_kernel_unittests();
    }

    #[cfg(target_pointer_width = "32")]
    const K: usize = 1;

    #[cfg(all(debug_assertions, target_pointer_width = "64"))]
    pub const K: usize = 1;
    #[cfg(all(not(debug_assertions), target_pointer_width = "64"))]
    pub const K: usize = 64;

    static TEST_THREAD_STORAGES: [SystemThreadStorage; NUM_CORES * K] =
        [const { SystemThreadStorage::new(ThreadKind::Normal) }; NUM_CORES * K];
    static mut TEST_THREADS: [MaybeUninit<ThreadNode>; NUM_CORES * K] =
        [const { MaybeUninit::zeroed() }; NUM_CORES * K];

    static MAIN_THREAD_STORAGE: SystemThreadStorage = SystemThreadStorage::new(ThreadKind::Normal);
    static mut MAIN_THREAD: MaybeUninit<ThreadNode> = MaybeUninit::zeroed();

    fn reset_and_queue_test_thread(
        i: usize,
        entry: extern "C" fn(),
        cleanup: Option<extern "C" fn()>,
    ) {
        unsafe {
            let t = TEST_THREADS[i].assume_init_ref();
            let mut w = t.lock();
            let stack = &TEST_THREAD_STORAGES[i].stack;
            let stack = thread::Stack::Raw {
                base: stack.rep.as_ptr() as usize,
                size: stack.rep.len(),
            };
            w.init(stack, thread::Entry::C(entry));
            if let Some(cleanup) = cleanup {
                w.set_cleanup(Entry::C(cleanup));
            };
            let ok = scheduler::queue_ready_thread(w.state(), t.clone());
            assert!(ok);
        }
    }

    fn reset_and_queue_test_threads(entry: extern "C" fn(), cleanup: Option<extern "C" fn()>) {
        unsafe {
            for i in 0..TEST_THREADS.len() {
                reset_and_queue_test_thread(i, entry, cleanup);
            }
        }
    }

    fn init_test_thread(i: usize) {
        let t = thread::build_static_thread(
            unsafe { &mut TEST_THREADS[i] },
            &TEST_THREAD_STORAGES[i],
            config::MAX_THREAD_PRIORITY / 2,
            thread::CREATED,
            Entry::C(test_main),
            ThreadKind::Normal,
        );
    }

    extern "C" fn init_test() {
        let t = thread::build_static_thread(
            unsafe { &mut MAIN_THREAD },
            &MAIN_THREAD_STORAGE,
            config::MAX_THREAD_PRIORITY / 2,
            thread::CREATED,
            Entry::C(test_main),
            ThreadKind::Normal,
        );
        let ok = scheduler::queue_ready_thread(thread::CREATED, t.clone());
        assert!(ok);
        let l = unsafe { TEST_THREADS.len() };
        debug!("Total test threads: {}", l);
        for i in 0..l {
            init_test_thread(i);
        }
    }

    #[cfg(target_pointer_width = "64")]
    const EMBALLOC_SIZE: usize = 8 << 20;
    #[cfg(target_pointer_width = "32")]
    const EMBALLOC_SIZE: usize = 2 << 20;

    #[global_allocator]
    static ALLOCATOR: KernelAllocator = KernelAllocator;
    // Emballoc is for correctness reference.
    //static ALLOCATOR: emballoc::Allocator<{ EMBALLOC_SIZE }> = emballoc::Allocator::new();

    #[panic_handler]
    fn oops(info: &PanicInfo) -> ! {
        let _ = DisableInterruptGuard::new();
        semihosting::println!("{}", info);
        semihosting::println!("Oops: {}", info.message());
        loop {}
    }

    #[test]
    fn test_rwlock() {
        let lock = types::RwLock::new(0);
        let mut w = lock.write();
        *w = 1;
        drop(w);

        assert!(scheduler::current_thread().validate_sp());
        scheduler::yield_me_now_or_later();
        assert!(scheduler::current_thread().validate_sp());

        let r = lock.read();
        assert_eq!(*r, 1);
    }

    #[test]
    fn test_spinlock() {
        let lock = sync::spinlock::SpinLock::new(0);
        let mut w = lock.irqsave_lock();
        *w = 1;
        drop(w);

        assert!(scheduler::current_thread().validate_sp());
        scheduler::yield_me_now_or_later();
        assert!(scheduler::current_thread().validate_sp());

        let r = lock.irqsave_lock();
        assert_eq!(*r, 1);
    }

    #[test]
    fn test_spinlock_loop() {
        let lock = sync::spinlock::SpinLock::new(0);
        loop {
            let mut w = lock.irqsave_lock();
            *w += 1;
            drop(w);

            scheduler::yield_me_now_or_later();

            let r = lock.irqsave_lock();
            if *r == 100 {
                break;
            }
        }
    }

    #[cfg(cortex_m)]
    #[test]
    fn test_sys_tick() {
        let tick = time::get_sys_ticks();
        assert!(scheduler::current_thread().validate_sp());
        scheduler::suspend_me_for(10);
        assert!(scheduler::current_thread().validate_sp());
        let tick2 = time::get_sys_ticks();
        assert!(tick2 - tick >= 10);
        assert!(tick2 - tick <= 11);
    }

    #[test]
    fn test_local_irq() {
        assert!(arch::local_irq_enabled());
    }

    #[test]
    fn stress_trap() {
        #[cfg(target_pointer_width = "32")]
        let n = 16;
        #[cfg(target_pointer_width = "64")]
        let n = 256;
        for _i in 0..n {
            #[cfg(any(target_arch = "riscv64", target_arch = "riscv32"))]
            unsafe {
                core::arch::asm!(
                    "ecall",
                    in("a7") Nop as usize,
                    inlateout("a0") 0 => _,
                    options(nostack),
                );
            };
        }
    }

    static mut SEMA_COUNTER: usize = 0usize;
    static SEMA: sync::semaphore::Semaphore = sync::semaphore::Semaphore::new(1);

    extern "C" fn test_semaphore() {
        SEMA.acquire_notimeout();
        let n = unsafe { SEMA_COUNTER };
        unsafe { SEMA_COUNTER += 1 };
        SEMA.release();
    }

    #[test]
    fn stress_semaphore() {
        SEMA.init();
        reset_and_queue_test_threads(test_semaphore, None);
        let l = unsafe { TEST_THREADS.len() };
        loop {
            SEMA.acquire_notimeout();
            let n = unsafe { SEMA_COUNTER };
            if n == l {
                SEMA.release();
                break;
            }
            SEMA.release();
            scheduler::yield_me();
        }
    }

    static TEST_ATOMIC_WAIT: AtomicUsize = AtomicUsize::new(0);

    extern "C" fn test_atomic_wait_cleanup() {
        TEST_ATOMIC_WAIT.fetch_add(1, Ordering::Release);
        sync::atomic_wait::atomic_wake(&TEST_ATOMIC_WAIT as *const _ as usize, 1);
    }

    extern "C" fn test_atomic_wait() {}

    #[test]
    fn stress_atomic_wait() {
        reset_and_queue_test_threads(test_atomic_wait, Some(test_atomic_wait_cleanup));
        let l = unsafe { TEST_THREADS.len() };
        loop {
            let n = TEST_ATOMIC_WAIT.load(Ordering::Acquire);
            if n == l {
                break;
            }
            sync::atomic_wait::atomic_wait(&TEST_ATOMIC_WAIT as *const _ as usize, n, None);
        }
    }

    static TEST_SWITCH_CONTEXT: AtomicUsize = AtomicUsize::new(0);

    extern "C" fn test_switch_context() {
        let n = 4;
        for _i in 0..n {
            assert!(scheduler::current_thread().validate_sp());
            scheduler::yield_me();
            assert!(scheduler::current_thread().validate_sp());
        }
    }

    extern "C" fn test_switch_context_cleanup() {
        TEST_SWITCH_CONTEXT.fetch_add(1, Ordering::Relaxed);
    }

    #[test]
    fn stress_context_switch() {
        reset_and_queue_test_threads(test_switch_context, Some(test_switch_context_cleanup));
        loop {
            let n = TEST_SWITCH_CONTEXT.load(Ordering::Relaxed);
            if n == unsafe { TEST_THREADS.len() } {
                break;
            }
            assert!(scheduler::current_thread().validate_sp());
            scheduler::yield_me();
            assert!(scheduler::current_thread().validate_sp());
        }
    }

    static BUILT_THREADS: AtomicUsize = AtomicUsize::new(0);

    extern "C" fn do_it() {}

    extern "C" fn do_cleanup() {
        BUILT_THREADS.fetch_add(1, Ordering::Relaxed);
    }

    #[test]
    fn stress_build_threads() {
        #[cfg(target_pointer_width = "32")]
        let n = 128;
        #[cfg(all(debug_assertions, target_pointer_width = "64"))]
        let n = 128;
        #[cfg(all(not(debug_assertions), target_pointer_width = "64"))]
        let n = 512;
        for _i in 0..n {
            let t = thread::Builder::new(thread::Entry::C(do_it)).build();
            t.lock().set_cleanup(thread::Entry::C(do_cleanup));
            let ok = scheduler::queue_ready_thread(t.state(), t);
            assert!(ok);
        }
        loop {
            let m = BUILT_THREADS.load(Ordering::Relaxed);
            if m == n {
                break;
            }
            scheduler::yield_me();
        }
    }

    static SPAWNED_THREADS: AtomicUsize = AtomicUsize::new(0);
    #[test]
    fn stress_spawn_threads() {
        #[cfg(target_pointer_width = "32")]
        let n = 32;
        #[cfg(all(debug_assertions, target_pointer_width = "64"))]
        let n = 32;
        #[cfg(all(not(debug_assertions), target_pointer_width = "64"))]
        let n = 512;
        for _i in 0..n {
            thread::spawn(move || {
                SPAWNED_THREADS.fetch_add(1, Ordering::Relaxed);
            });
        }
        loop {
            let m = SPAWNED_THREADS.load(Ordering::Relaxed);
            if m == n {
                break;
            }
            scheduler::yield_me();
        }
    }

    async fn foo(i: usize) -> usize {
        i
    }

    async fn bar() -> usize {
        42
    }

    async fn is_asynk_working() {
        let a = foo(42).await;
        let b = bar().await;
        assert_eq!(a - b, 0);
    }

    // FIXME: asynk runtime not stable yet.
    // #[test]
    // fn stress_async_basic() {
    //     let n = 1024;
    //     for _i in 0..n {
    //         asynk::block_on(is_asynk_working());
    //     }
    // }

    #[inline(never)]
    pub fn kernel_unittest_runner(tests: &[&dyn Fn()]) {
        let t = scheduler::current_thread();
        semihosting::println!("---- Running {} kernel unittests...", tests.len());
        semihosting::println!(
            "Before test, thread 0x{:x}, rc: {}, heap status: {:?}, sp: 0x{:x}",
            Thread::id(&t),
            ThreadNode::strong_count(&t),
            ALLOCATOR.memory_info(),
            arch::current_sp(),
        );
        for test in tests {
            test();
        }
        semihosting::println!(
            "After test, thread 0x{:x}, heap status: {:?}, sp: 0x{:x}",
            Thread::id(&t),
            ALLOCATOR.memory_info(),
            arch::current_sp()
        );
        semihosting::println!("---- Done kernel unittests.");
    }
}
