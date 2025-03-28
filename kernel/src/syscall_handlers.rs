mod platform;
use crate::{
    c_str,
    clock::tick_from_millisecond,
    cpu::Cpu,
    sync::futex,
    thread::{ThreadBuilder, THREAD_DEFAULT_TICK},
};
use bluekernel_header::{syscalls::NR, thread::CloneArgs};
use libc::timespec;

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
				#[inline]
        pub(crate) extern "C" fn dispatch_syscall(ctx: &Context) -> usize {
            match ctx.nr {
                $(val if val == NR::$nr as usize => unsafe { $crate::syscall_handlers::$mod::handle_context(ctx) })*
                _ => usize::MAX
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
        let $arg = core::mem::transmute::<usize, $argty>($args[$idx]);
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

						#[cfg(direct_syscall_handler)]
						#[inline]
            // This must be `pub` since this is the entry of direct invoking of syscall handler.
            pub extern "C" fn handle($($arg: $argty),*) -> $ret {
                $body
            }

						// FIXME: Rustc miscompiles it if #[inline] here.
						#[cfg(not(direct_syscall_handler))]
            pub extern "C" fn handle($($arg: $argty),*) -> $ret {
                $body
            }

            #[inline]
            pub unsafe extern "C" fn handle_context(_ctx: &Context) -> usize {
                map_args!(_ctx.args, 0 $(, $arg, $argty)*);
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
exit_thread() -> c_long {
    unsafe { crate::thread::Thread::exit() };
    // If control reaches here, definitely error occurs, return -1 to indicate that.
    return -1;
});

define_syscall_handler!(
atomic_wait(addr: usize, val: usize, timeout: Option<&timespec>) -> c_long {
    let timeout = timeout.map_or(-1, |t| t.tv_sec * 1000 + t.tv_nsec / 1000000);
    let timeout = tick_from_millisecond(timeout) as i32;
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

syscall_table! {

(Echo, echo),
(Nop, nop),
(GetTid, get_tid),
(CreateThread, create_thread),
(ExitThread, exit_thread),
(AtomicWake, atomic_wake),
(AtomicWait, atomic_wait),

}

// Begin syscall modules.
pub mod echo;
// End syscall modules.
