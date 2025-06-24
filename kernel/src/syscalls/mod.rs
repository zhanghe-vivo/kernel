use crate::{
    arch, scheduler,
    sync::atomic_wait as futex,
    thread,
    thread::{Builder, Entry, Stack, Thread, ThreadNode},
    trace,
    vfs::posix as vfs_posix,
};
use bluekernel_header::{syscalls::NR, thread::CloneArgs};
use libc::{c_char, c_int, c_void, clockid_t, mode_t, size_t, timespec, EINVAL};

#[repr(C)]
#[derive(Default)]
pub struct Context {
    pub nr: usize,
    pub args: [usize; 6],
}

// For every syscall number in NR, we have to define a module to
// handle the syscall request.  `handle_context` serves as the
// dispatcher if syscall is invoked via software interrupt.
// bk_syscall! macro should be used by external libraries if syscall
// is invoked via function call.
macro_rules! syscall_table {
    ($(($nr:tt, $mod:ident),)*) => {
        pub(crate) fn dispatch_syscall(ctx: &Context) -> usize {
            match ctx.nr {
                $(val if val == NR::$nr as usize =>
                    return $crate::syscalls::$mod::handle_context(ctx) as usize,)*
                _ => return usize::MAX,
            }
        }

        #[macro_export]
        macro_rules! bk_syscall {
            $(($nr $$(,$arg:expr)*) => { $crate::syscalls::$mod::handle($$($arg),*) });*
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
    let t = scheduler::current_thread();
    let handle = Thread::id(&t);
    return handle as c_long;
});

define_syscall_handler!(
create_thread(clone_args_ptr: *const CloneArgs) -> c_long {
    let clone_args = unsafe {&*clone_args_ptr};
    let t = thread::Builder::new(Entry::Posix(clone_args.entry, clone_args.arg))
        .set_stack(Stack::Raw{base:clone_args.stack_start as usize, size: clone_args.stack_size})
        .build();
    let handle = Thread::id(&t);
    clone_args.clone_hook.map(|f| {
        f(handle, clone_args);
    });
    let ok = scheduler::queue_ready_thread(thread::CREATED, t);
    // We don't increment the rc of the created thread since it's also
    // referenced by the global queue. When this thread is retired,
    // it's removed from the global queue.
    assert!(ok);
    return unsafe {core::mem::transmute(handle)};
});

define_syscall_handler!(
atomic_wait(addr: usize, val: usize, timeout: *const timespec) -> c_long {
    let timeout = if timeout.is_null() {
        None
    } else {
        unsafe { Some(&*timeout) }
    };
    futex::atomic_wait(addr, val, None).map_or_else(|e|e as c_long, |_| 0)
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
        vfs_posix::vfs_write(
        fd,
        buf, size) as c_long
    }
});

define_syscall_handler!(open(path: *const c_char, flags: c_int, mode: mode_t) -> c_int {
    vfs_posix::vfs_open(path, flags, mode)
});

define_syscall_handler!(
    close(fd: c_int) -> c_int {
        vfs_posix::vfs_close(fd)
    }
);
define_syscall_handler!(
    read(fd: c_int, buf: *mut c_void, count: size_t) -> isize {
        vfs_posix::vfs_read(fd, buf as *mut u8, count as usize)
    }
);

define_syscall_handler!(
    lseek(fildes: c_int, offset: usize, whence: c_int) -> c_int {
        vfs_posix::vfs_lseek(fildes, offset as i64, whence) as c_int
    }
);

define_syscall_handler!(exit_thread() -> c_long {
    scheduler::retire_me();
});

define_syscall_handler!(sched_yield() -> c_long {
    scheduler::yield_me();
    0
});

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
    (Open, open),
    (Lseek, lseek),
    (SchedYield, sched_yield),
}

// Begin syscall modules.
pub mod echo;
// End syscall modules.
