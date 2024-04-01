#![no_std]

extern crate cty;

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
