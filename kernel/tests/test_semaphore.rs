use bluekernel::{
    println,
    sync::{semaphore::*, wait_list::*},
};
use bluekernel_infra::custom_test;

custom_test! {
    fn test_sempahore_init() {
        let _sem = Semaphore::new("sem1", 3, WaitMode::Fifo);
    }
}
