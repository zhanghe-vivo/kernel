mod platform;
use bluekernel_header::syscalls::NR;

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
                      -> $ret:ty {
        $($body:stmt);*
    }) => (
        pub mod $handler {
            use super::Context;
            use core::ffi::c_long;

            // This must be `pub` since this is the entry of direct invoking of syscall handler.
            #[inline]
            pub extern "C" fn handle($($arg: $argty),*) -> $ret {
                $($body);*
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

syscall_table! {

(Echo, echo),
(Nop, nop),

}

// Begin syscall modules.
pub mod echo;
// End syscall modules.
