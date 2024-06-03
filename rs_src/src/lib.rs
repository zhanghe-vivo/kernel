#![feature(panic_info_message)]
#![allow(internal_features)]
#![feature(core_intrinsics)]
#![feature(linkage)]
#![no_std]

#[macro_use]
mod static_assert;
#[macro_use]
mod print;

#[cfg(feature = "RT_DEBUGING_SPINLOCK")]
#[macro_use]
mod caller_address;

mod alloc;
mod clock;
mod cpu;
mod irq;
mod linked_list;
mod object;
mod rt_bindings;
mod rt_list;
mod sync;

#[cfg(feature = "RT_USING_SMP")]
mod cpu;

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
