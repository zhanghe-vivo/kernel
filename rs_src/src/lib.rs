#![no_std]

#[macro_use]
mod static_assert;

#[cfg(feature = "RT_DEBUGING_SPINLOCK")]
#[macro_use]
mod caller_address;

extern crate cty;

mod clock;
mod cpu;
mod object;
mod rt_bindings;
mod rt_list;

use core::panic::PanicInfo;
use core::sync::atomic::{self, Ordering};

// TODO: write a panic by rtthread
#[inline(never)]
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        atomic::compiler_fence(Ordering::SeqCst);
    }
}
