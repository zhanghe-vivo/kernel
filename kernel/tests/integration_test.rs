// NEWLINE-TIMEOUT: 15
// ASSERT-SUCC: Kernel integration test end.
// ASSERT-FAIL: Backtrace in Panic.*
#![no_main]
#![no_std]
#![feature(custom_test_frameworks)]
#![test_runner(kernel_test_runner)]
#![reexport_test_harness_main = "kernel_test_main"]

extern crate alloc;
use bluekernel::{allocator, println};

mod test_futex;
/// Unstable rust custom test framework test file hierarchy.
/// Since there is no cargo framework, we manually set it up.
mod test_semaphore;
mod test_vfs;

/// Unstable rust custom test framework test runner
pub fn kernel_test_runner(tests: &[&dyn Fn()]) {
    println!("Kernel integration test start...");
    println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
    println!("Kernel integration test end.");
}

#[no_mangle]
fn main() -> i32 {
    println!("Hello, Blue Kernel!");

    let memory_info = allocator::memory_info();
    println!("Kernel memory statistics: ");
    println!("Total memory: {} bytes.", memory_info.total);
    println!("Used memory: {} bytes.", memory_info.used);
    println!("Max used memory: {} bytes.", memory_info.max_used);

    // Unstable rust custom test framework tests entry.
    kernel_test_main();

    #[cfg(coverage)]
    bluekernel::cov::write_coverage_data();
    0
}
