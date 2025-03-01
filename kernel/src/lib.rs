#![no_std]
#![allow(internal_features)]
#![feature(core_intrinsics)]
#![feature(link_llvm_intrinsics)]
#![feature(linkage)]
#![feature(allocator_api)]
#![feature(alloc_layout_extra)]
#![feature(c_size_t)]

pub extern crate alloc;
extern crate self as kernel;
pub use bluekernel_arch;
pub use bluekernel_kconfig;

pub mod allocator;
pub mod clock;
pub mod components;
pub mod cpu;
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
pub mod startup;
pub mod static_init;
pub mod sync;
pub mod thread;
pub mod timer;
mod zombie;
#[allow(unused_imports)]
use core::sync::atomic::{self, Ordering};

#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    if cpu::Cpus::is_inited() {
        cpu::Cpus::lock_cpus();
    }
    println!("{}", info);

    println!(
        "Backtrace in Panic: {}",
        bluekernel_arch::arch::Arch::backtrace()
    );

    #[cfg(debug_assertions)]
    loop {
        atomic::compiler_fence(Ordering::SeqCst);
    }
    #[cfg(not(debug_assertions))]
    {
        bluekernel_arch::arch::Arch::sys_reset()
    }
}

/// Macro to check current context.
#[cfg(feature = "debugging_context")]
#[macro_export]
macro_rules! debug_not_in_interrupt {
    () => {
        use crate::cpu;
        use bluekernel_arch::IInterrupt;

        let level = bluekernel_arch::arch::Arch::disable_interrupts();
        if cpu::Cpu::interrupt_nest_load() != 0 {
            crate::kprintf!(
                b"Function[%s] shall not be used in ISR\n",
                crate::function!() as *const _ as *const i8,
            );
            assert!(false);
        }
        bluekernel_arch::arch::Arch::enable_interrupts(level);
    };
}

///  "In thread context" means:
///    1) the scheduler has been started
///    2) not in interrupt context.
#[cfg(feature = "debugging_context")]
#[macro_export]
macro_rules! debug_in_thread_context {
    () => {
        let level = bluekernel_arch::arch::Arch::disable_interrupts();
        if cpu::Cpu::get_current_thread().is_none() {
            assert!(false);
        }
        kernel::debug_not_in_interrupt!();
        bluekernel_arch::arch::Arch::enable_interrupts(level);
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

            let interrupt_disabled = !bluekernel_arch::arch::Arch::is_interrupts_active();
            let level = bluekernel_arch::arch::Arch::disable_interrupts();
            if cpu::Cpu::get_current_scheduler().get_sched_lock_level() != 0 {
                crate::kprintf!(
                    b"Function[%s]: scheduler is not available\n",
                    crate::function!() as *const _ as *const i8,
                );
                assert!(false);
            }
            if interrupt_disabled {
                crate::kprintf!(
                    b"Function[%s]: interrupt is disabled\n",
                    crate::function!() as *const _ as *const i8,
                );

                assert!(false);
            }
            kernel::debug_in_thread_context!();
            bluekernel_arch::arch::Arch::enable_interrupts(level);
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
