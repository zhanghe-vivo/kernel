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
mod allocator;
#[cfg(feature = "RT_DEBUGING_SPINLOCK")]
mod caller_address;
pub mod clock;
pub mod components;
pub mod cpu;
pub mod error;
mod ext_types;
mod idle;
pub mod irq;
pub mod klibc;
mod linked_list;
pub mod object;
mod print;
mod rt_bindings;
mod rt_list;
pub mod scheduler;
mod stack;
mod static_assert;
pub mod static_init;
pub mod str;
pub mod sync;
pub mod thread;
mod zombie;

// need to call before rt_enter_critical/ cpus_lock called
#[no_mangle]
pub unsafe extern "C" fn init_cpus() {
    object::OBJECT_CONTAINER.init_once();
    cpu::CPUS.init_once();
}

use core::sync::atomic::{self, Ordering};
#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    cpu::Cpus::lock_cpus();
    println!("{}", info);
    unsafe {
        rt_bindings::rt_hw_cpu_reset(); // may return
    }
    #[cfg(debug_assertions)]
    loop {
        atomic::compiler_fence(Ordering::SeqCst);
    }
    #[cfg(not(debug_assertions))]
    unsafe {
        core::intrinsics::abort()
    }
}
