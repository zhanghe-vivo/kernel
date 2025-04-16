// NEWLINE-TIMEOUT: 15
// ASSERT-SUCC: Kernel unit test end.
// ASSERT-FAIL: Backtrace in Panic.*
#![no_std]
#![allow(internal_features)]
#![feature(core_intrinsics)]
#![feature(link_llvm_intrinsics)]
#![feature(linkage)]
#![feature(allocator_api)]
#![feature(alloc_layout_extra)]
#![feature(c_size_t)]
#![feature(alloc_error_handler)]
#![feature(c_variadic)]
#![feature(naked_functions)]
#![feature(macro_metavar_expr)]
#![feature(pointer_is_aligned_to)]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(kernel_utest_runner)]
#![reexport_test_harness_main = "kernel_utest_main"]

pub extern crate alloc;
pub use bluekernel_arch::arch;
pub use bluekernel_kconfig;
pub use libc;
#[cfg(feature = "os_adapter")]
pub use os_bindings;

pub mod allocator;
mod bsp;
pub mod clock;
pub mod cpu;
pub mod drivers;
pub mod error;
mod ext_types;
pub mod idle;
pub mod irq;
mod macros;
pub mod object;
pub mod print;
pub mod process;
pub mod scheduler;
mod stack;
mod startup;
mod static_init;
pub mod sync;
#[cfg(not(direct_syscall_handler))]
mod syscall_handlers;
#[cfg(direct_syscall_handler)]
pub mod syscall_handlers;
pub mod thread;
pub mod timer;
pub mod vfs;
mod zombie;

// #[link_section] is only usable from the root crate.
// See https://github.com/rust-lang/rust/issues/67209.
#[cfg(target_board = "qemu_mps2_an385")]
include!("bsp/qemu_mps2_an385/handlers.rs");
#[cfg(target_board = "qemu_mps3_an547")]
include!("bsp/qemu_mps3_an547/handlers.rs");

#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    if cpu::Cpus::is_inited() {
        cpu::Cpus::lock_cpus();
    }
    println!("{}", info);

    println!("Backtrace in Panic: {}", arch::Arch::backtrace());

    #[cfg(debug_assertions)]
    loop {
        use core::sync::atomic::{self, Ordering};
        atomic::compiler_fence(Ordering::SeqCst);
    }
    #[cfg(not(debug_assertions))]
    {
        arch::Arch::sys_reset()
    }
}

/// Macro to check current context.
#[cfg(feature = "debugging_context")]
#[macro_export]
macro_rules! debug_not_in_interrupt {
    () => {
        use crate::cpu;

        let level = arch::Arch::disable_interrupts();
        if cpu::Cpu::interrupt_nest_load() != 0 {
            unreachable!(
                "Function[{}] shall not be used in ISR",
                crate::function_name!()
            );
        }
        arch::Arch::enable_interrupts(level);
    };
}

///  "In thread context" means:
///    1) the scheduler has been started
///    2) not in interrupt context.
#[cfg(feature = "debugging_context")]
#[macro_export]
macro_rules! debug_in_thread_context {
    () => {
        let level = arch::Arch::disable_interrupts();
        if cpu::Cpu::get_current_thread().is_none() {
            unreachable!("current_thread is none!");
        }
        kernel::debug_not_in_interrupt!();
        arch::Arch::enable_interrupts(level);
    };
}

/// "scheduler available" means:
/// 1) the scheduler has been started.
/// 2) not in interrupt context.
/// 3) scheduler is not locked.
/// 4) interrupt is not disabled.
#[cfg(feature = "debugging_context")]
#[macro_export]
macro_rules! debug_scheduler_available {
    ($need_check:expr) => {{
        if $need_check {
            use crate::irq;

            let interrupt_disabled = !arch::Arch::is_interrupts_active();
            let level = arch::Arch::disable_interrupts();
            if cpu::Cpu::get_current_scheduler().get_sched_lock_level() != 0 {
                unreachable!(
                    "Function[{}] scheduler is not available",
                    crate::function_name!()
                );
            }
            if interrupt_disabled {
                unreachable!(
                    "Function[{}] interrupt is disabled",
                    crate::function_name!()
                );
            }
            kernel::debug_in_thread_context!();
            arch::Arch::enable_interrupts(level);
        }
    }};
}

#[cfg(not(feature = "debugging_context"))]
#[macro_export]
macro_rules! debug_not_in_interrupt {
    () => {};
}
#[cfg(not(feature = "debugging_context"))]
#[macro_export]
macro_rules! debug_in_thread_context {
    () => {};
}
#[cfg(not(feature = "debugging_context"))]
#[macro_export]
macro_rules! debug_scheduler_available {
    ($need_check:expr) => {};
}

#[cfg(test)]
pub fn utest_main() {
    #[cfg(test)]
    kernel_utest_main();
}

#[cfg(test)]
pub fn kernel_utest_runner(tests: &[&dyn Fn()]) {
    println!("Kernel unit test start...");
    println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
    println!("Kernel unit test end.");
}
