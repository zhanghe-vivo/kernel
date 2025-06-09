mod platform;
use crate::{
    c_str,
    clock::{tick_from_millisecond, tick_get_millisecond},
    cpu::Cpu,
    sync::futex,
    thread::{ThreadBuilder, THREAD_DEFAULT_TICK},
    vfs::vfs_posix,
};
use bluekernel_header::{syscalls::NR, thread::CloneArgs};
use libc::{c_int, c_void, clockid_t, size_t, timespec};

#[repr(C)]
#[derive(Default)]
pub struct Context {
    pub nr: usize,
    pub args: [usize; 6],
}

/// For every syscall number in NR, we have to define a module to handle the syscall request.
/// `handle_context` serves as the dispatcher if syscall is invoked via software interrupt.
/// bk_syscall! macro should be used by external libraries if syscall is invoked via function call.
macro_rules! syscall_table {
    ($(($nr:tt, $mod:ident),)*) => {
        pub(crate) fn dispatch_syscall(ctx: &Context) -> usize {
            match ctx.nr {
                $(val if val == NR::$nr as usize =>
                    return $crate::syscall_handlers::$mod::handle_context(ctx) as usize,)*
                _ => return usize::MAX,
            }
        }

        #[macro_export]
        macro_rules! bk_syscall {
            $(($nr $$(,$arg:expr)*) => { $crate::syscall_handlers::$mod::handle($$($arg),*) });*
        }
    };
}

macro_rules! map_args {
    ($args:expr, $idx:expr) => {};
    ($args:expr, $idx:expr, $arg:ident, $argty:ty $(, $tailarg:ident, $tailargty:ty)*) => {
        let $arg = unsafe { core::mem::transmute_copy::<usize, $argty>(&$args[$idx]) };
        map_args!($args, $idx+1 $(, $tailarg, $tailargty)*);
    };
}

// A helper macro to implement syscall handler quickly.
macro_rules! define_syscall_handler {
    ($handler:ident($($arg:ident: $argty:ty),*)
                    -> $ret:ty $body:block
    ) => (
        pub mod $handler {
            use super::*;
            use core::ffi::c_long;

            // FIXME: Rustc miscompiles if inlined.
            #[inline(never)]
            pub fn handle($($arg: $argty),*) -> $ret {
                $body
            }

            pub fn handle_context(ctx: &Context) -> usize {
                map_args!(ctx.args, 0 $(, $arg, $argty)*);
                handle($($arg),*) as usize
            }
        }
    )
}

define_syscall_handler!(
nop() -> c_long {
    0
});

define_syscall_handler!(
get_tid() -> c_long {
    Cpu::get_current_thread().map_or(-1, |t| t.as_ptr() as c_long)
});

define_syscall_handler!(
create_thread(clone_args_ptr: *const CloneArgs) -> c_long {
    let clone_args = unsafe {&*clone_args_ptr};
    let builder = ThreadBuilder::default();
    let thread = builder
        .stack_start(clone_args.stack_start)
        .stack_size(clone_args.stack_size)
        .name(c_str!("posix"))
        .entry_fn(clone_args.entry)
        .arg(clone_args.arg)
        .tick(THREAD_DEFAULT_TICK)
        .build_from_heap();
    thread.map_or(-1, |mut t| {
        // TODO: clone_hook is user space function, should switch to user mode to execute it.
        clone_args.clone_hook.map(|f| f(t.as_ptr() as usize, clone_args));
        unsafe { t.as_mut().start() };
        t.as_ptr() as c_long
    })
});

define_syscall_handler!(
atomic_wait(addr: usize, val: usize, timeout: *const timespec) -> c_long {
    let timeout = if timeout.is_null() {
        None
    } else {
        unsafe { Some(&*timeout) }
    };
    let timeout = timeout.map_or(-1, |t| t.tv_sec * 1000 + t.tv_nsec / 1000000);
    let timeout = tick_from_millisecond(timeout);
    futex::atomic_wait(addr, val, timeout).map_or_else(|e|e as c_long, |_| 0)
});

define_syscall_handler!(
atomic_wake(addr: usize, count: *mut usize) -> c_long {
    let how_many = unsafe { *count };
    futex::atomic_wake(addr, how_many).map_or_else(|_| -1, |woken| {
        unsafe { *count = woken };
        0
    })
});

// Only for posix testsuite, we need to implement a stub for clock_gettime
define_syscall_handler!(
    clock_gettime(_clk_id: clockid_t, tp: *mut timespec) -> c_long {
        let mut time = timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        // Don't care about clk_id for now
        let t: i64 = (tick_get_millisecond()  as i64) * 1000 * 1000;
        time.tv_sec = (t / (1000 * 1000 * 1000)) as i32 ;
        time.tv_nsec = (t % (1000 * 1000 * 1000)) as i32;

        unsafe { *tp = time };

        0
});

define_syscall_handler!(
alloc_mem(ptr: *mut *mut c_void, size: usize, align: usize) -> c_long {
    let addr = crate::allocator::malloc_align(size, align);
    if addr.is_null() {
        return -1;
    }
    unsafe { ptr.write(addr as *mut c_void) };
    return 0;
});

define_syscall_handler!(
free_mem(ptr: *mut c_void) -> c_long {
    crate::allocator::free(ptr as *mut u8);
    return 0;
});

define_syscall_handler!(
write(fd: i32, buf: *const u8, size: usize) -> c_long {
    unsafe {
        crate::vfs::vfs_posix::write(
        fd,
        core::slice::from_raw_parts(buf, size), size) as c_long
    }
});

define_syscall_handler!(
    close(fd: c_int) -> c_int {
        vfs_posix::close(fd)
    }
);
define_syscall_handler!(
    read(fd: c_int, buf: *mut c_void, count: size_t) -> isize {
        vfs_posix::read(fd, buf as *mut core::ffi::c_void, count as usize)
    }
);

syscall_table! {
    (Echo, echo),
    (Nop, nop),
    (GetTid, get_tid),
    (CreateThread, create_thread),
    (ExitThread, exit_thread),
    (AtomicWake, atomic_wake),
    (AtomicWait, atomic_wait),
    // For test only
    (ClockGetTime, clock_gettime),
    (AllocMem, alloc_mem),
    (FreeMem, free_mem),
    (Write, write),
    (Close, close),
    (Read, read),
}

// Begin syscall modules.
pub mod echo;
pub mod exit_thread;
// End syscall modules.
