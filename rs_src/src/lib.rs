#![no_std]
#![feature(link_llvm_intrinsics)]

#[macro_use]
mod static_assert;
#[macro_use]
mod caller_address;

extern crate cty;

mod clock;
mod cpu;

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
