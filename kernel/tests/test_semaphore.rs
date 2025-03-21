use bluekernel_test as kernel;
use kernel::{println, sync::{semaphore::*, wait_list::*}};

#[test_case]
fn test_sempahore_init() {
    println!("test_semaphore_init...");
    let _sem = Semaphore::new("sem1", 3, WaitMode::Fifo);
    println!("[ok]");
}