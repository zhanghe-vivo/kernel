#![no_main]
#![no_std]
use bluekernel2 as kernel;
use kernel::{allocator, println, thread::Thread};

#[no_mangle]
fn main() -> i32 {
    println!("Hello, Blue OS!");

    let (total, used, max_used) = allocator::memory_info();
    println!("Total memory: {} bytes", total);
    println!("Used memory: {} bytes", used);
    println!("Max used memory: {} bytes", max_used);

    loop {
        let _ = Thread::msleep(1000);
    }
}
