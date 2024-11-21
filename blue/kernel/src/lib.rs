#![no_std]
#![feature(panic_info_message)]
#![allow(internal_features)]
#![feature(core_intrinsics)]
#![feature(link_llvm_intrinsics)]
#![feature(linkage)]
#![feature(allocator_api)]
#![feature(alloc_layout_extra)]
#![feature(c_size_t)]

extern crate alloc;
extern crate self as kernel;
pub use blue_arch;
#[allow(unused_imports)]
use blue_arch::arch as _;
// TODO: add os compat cfg
pub use rt_bindings;

mod allocator;
pub mod clock;
pub mod components;
pub mod cpu;
pub mod error;
mod ext_types;
mod idle;
pub mod irq;
pub mod klibc;
mod macros;
pub mod object;
mod print;
pub mod process;
pub mod scheduler;
mod stack;
pub mod static_init;
pub mod str;
pub mod sync;
pub mod thread;
pub mod timer;
mod zombie;

use core::sync::atomic::{self, Ordering};

#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    if unsafe { cpu::CPUS.is_inited() } {
        cpu::Cpus::lock_cpus();
    }
    println!("{}", info);
    #[cfg(debug_assertions)]
    loop {
        atomic::compiler_fence(Ordering::SeqCst);
    }
    #[cfg(not(debug_assertions))]
    {
        use blue_arch::IInterrupt;
        blue_arch::arch::Arch::sys_reset()
    }
}
