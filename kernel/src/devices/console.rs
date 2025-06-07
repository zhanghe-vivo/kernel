use super::{
    serial::{config::SerialConfig, Serial, UartOps},
    Device, DeviceManager,
};
use crate::{sync::SpinLock, vfs::vfs_mode::AccessMode};
use alloc::sync::Arc;
use embedded_io::ErrorKind;
use spin::Once;

static CONSOLE: Once<Arc<dyn Device>> = Once::new();

pub fn init_console(serial: &Arc<Serial>) -> Result<(), ErrorKind> {
    CONSOLE.call_once(|| serial.clone());
    DeviceManager::get().register_device("console", serial.clone())
}

pub fn get_console() -> Arc<dyn Device> {
    CONSOLE.get().unwrap().clone()
}

pub fn get_early_uart() -> &'static SpinLock<dyn UartOps> {
    crate::boards::uart::get_early_uart()
}
