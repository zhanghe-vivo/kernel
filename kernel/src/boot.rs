use crate::{allocator, arch, asynk, boards, logger, scheduler, thread, time, vfs};
use core::ptr::{addr_of, addr_of_mut};

pub(crate) static mut INIT_BSS_DONE: bool = false;
pub(crate) static mut INIT_ARRAY_DONE: bool = false;
pub(crate) static mut INIT_HEAP_DONE: bool = false;
pub(crate) static mut INIT_VFS_DONE: bool = false;

// See https://github.com/rust-lang/rust/pull/134213 for more details about naked function.
#[no_mangle]
#[naked]
pub unsafe extern "C" fn _start() {
    // Arch is responsible to init cores. After initialiing
    // cores, arch_bootstrap should continue with `init`.
    crate::arch_bootstrap!(__sys_stack_start, __sys_stack_end, init);
}

extern "C" {
    pub static __init_array_start: extern "C" fn();
    pub static __init_array_end: extern "C" fn();
    // Apps' entries should be put in bk_app_array section.
    pub static __bk_app_array_start: extern "C" fn();
    pub static __bk_app_array_end: extern "C" fn();
    pub static mut __bss_start: u8;
    pub static mut __bss_end: u8;
    pub static mut __sys_stack_start: u8;
    pub static mut __sys_stack_end: u8;
    pub static mut __heap_start: u8;
    pub static mut __heap_end: u8;
    pub static mut _end: u8;
}

extern "C" fn init() {
    boards::init();
    init_runtime();
    init_heap();
    scheduler::init();
    // FIXME: remove this after aarch64 and riscv64 is supported
    #[cfg(cortex_m)]
    logger::logger_init();
    time::timer::system_timer_init();
    asynk::init();
    init_vfs();
    init_apps();
    arch::start_schedule(scheduler::schedule);
    unreachable!("We should have jumped to the schedule loop!");
}

pub(crate) fn init_runtime() {
    init_bss();
    run_init_array();
}

pub(crate) fn init_vfs() {
    unsafe {
        if INIT_VFS_DONE {
            return;
        }
        if let Err(err) = vfs::vfs_init() {
            panic!("{}", err);
        };
        INIT_VFS_DONE = true;
    }
}

#[inline]
fn init_bss() {
    unsafe {
        if INIT_BSS_DONE {
            return;
        }
        // FIXME: Use memset?
        let mut ptr = addr_of_mut!(__bss_start);
        while ptr != addr_of_mut!(__bss_end) {
            ptr.write(0u8);
            ptr = ptr.offset(1);
        }
        INIT_BSS_DONE = true;
    }
}

#[inline]
fn run_init_array() {
    unsafe {
        if INIT_ARRAY_DONE {
            return;
        }
        let mut my_init = addr_of!(__init_array_start);
        while my_init < addr_of!(__init_array_end) {
            (*my_init)();
            my_init = my_init.offset(1);
        }
        INIT_ARRAY_DONE = true;
    }
}

#[inline(never)]
fn init_apps() {
    unsafe {
        let mut app = addr_of!(__bk_app_array_start);
        while app < addr_of!(__bk_app_array_end) {
            thread::Builder::new(thread::Entry::C(*app)).start();
            app = app.offset(1);
        }
    }
}

#[inline(never)]
pub(crate) fn init_heap() {
    unsafe {
        if INIT_HEAP_DONE {
            return;
        }
        allocator::init_heap(addr_of_mut!(__heap_start), addr_of_mut!(__heap_end));
        INIT_HEAP_DONE = true;
    }
}
