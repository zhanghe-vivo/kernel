#![no_std]
#![allow(internal_features)]
#![feature(core_intrinsics)]
#![feature(link_llvm_intrinsics)]
#![feature(linkage)]
#![feature(allocator_api)]
#![feature(alloc_layout_extra)]
#![feature(c_size_t)]

extern crate alloc;
extern crate self as kernel;
pub use blue_arch::arch::Arch;
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
    if cpu::Cpus::is_inited() {
        cpu::Cpus::lock_cpus();
    }
    println!("{}", info);
    #[cfg(debug_assertions)]
    loop {
        atomic::compiler_fence(Ordering::SeqCst);
    }
    #[cfg(not(debug_assertions))]
    {
        Arch::sys_reset()
    }
}
