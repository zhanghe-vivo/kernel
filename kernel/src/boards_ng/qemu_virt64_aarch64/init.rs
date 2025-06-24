// use super::{sys_config, systick::Systick};
use super::uart;
use crate::devices::{console, dumb, DeviceManager};
use alloc::string::ToString;
use core::ptr::addr_of_mut;

pub const NUM_CORES: usize = 1;

pub fn current_ticks() -> u64 {
    0
}

pub(crate) fn init() {
    crate::boot::init_runtime();
    unsafe { crate::boot::init_heap() };
    register_devices_in_vfs();
}

fn register_devices_in_vfs() {
    console::init_console(dumb::get_serial0());
    DeviceManager::get().register_device("ttyS0".to_string(), dumb::get_serial0().clone());
}
