#![no_main]
#![no_std]
#![feature(custom_test_frameworks)]
#![test_runner(kernel_test_runner)]
#![reexport_test_harness_main = "kernel_test_main"]

use bluekernel_test as kernel;
use kernel::{allocator, println, thread::Thread};

/// Unstable rust custom test framework test file hierarchy.
/// Since there is no cargo framework, we manually set it up.
mod test_semaphore;

/// Unstable rust custom test framework test runner
pub fn kernel_test_runner(tests: &[&dyn Fn()]) {
    println!("Kernel test start...");
    println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
    println!("Kernel test end.");
}

#[no_mangle]
fn main() -> i32 {
    println!("Hello, Blue Kernel!");

    let (total, used, max_used) = allocator::memory_info();
    println!("Kernel memory statistics: ");
    println!("Total memory: {} bytes.", total);
    println!("Used memory: {} bytes.", used);
    println!("Max used memory: {} bytes.", max_used);

    // Unstable rust custom test framework tests entry.
    kernel_test_main();

    loop {
        let _ = Thread::msleep(1000);
    }
}
