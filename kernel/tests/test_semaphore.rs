use bluekernel::{
    println,
    sync::{semaphore::*, wait_list::*},
};
use bluekernel_test_macro::test;

#[test]
fn test_sempahore_init() {
    let _sem = Semaphore::new("sem1", 3, WaitMode::Fifo);
}
