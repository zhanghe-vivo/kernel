#![no_std]
#![feature(panic_info_message)]
#![allow(internal_features)]
#![feature(core_intrinsics)]
#![feature(linkage)]
#![feature(allocator_api)]
#![feature(alloc_layout_extra)]
#![feature(c_size_t)]

extern crate alloc;

extern crate self as kernel;
mod rt_bindings;
mod allocator;
pub mod error;
pub mod clock;
pub mod cpu;
pub mod irq;
pub mod object;
pub mod klibc;
pub mod str;
pub mod sync;
pub mod static_init;
mod ext_types;
mod linked_list;
mod rt_list;
mod static_assert;
mod print;
#[cfg(feature = "RT_DEBUGING_SPINLOCK")]
mod caller_address;

use core::sync::atomic::{self, Ordering};

#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
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
