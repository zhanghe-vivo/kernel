#[cfg(line_discipline)]
use super::tty::n_tty::Tty;
use super::{dumb, tty::serial::UartOps, Device, DeviceManager};
use crate::sync::SpinLock;
use alloc::{string::String, sync::Arc};
use embedded_io::ErrorKind;
use spin::Once;

static CONSOLE: Once<Arc<dyn Device>> = Once::new();

pub fn init_console(device: Arc<dyn Device>) -> Result<(), ErrorKind> {
    CONSOLE.call_once(|| device.clone());
    DeviceManager::get().register_device(String::from("console"), device.clone())
}

pub fn get_console() -> Arc<dyn Device> {
    CONSOLE.get().unwrap().clone()
}

pub fn get_early_uart() -> &'static SpinLock<dyn UartOps> {
    get_early_uart()
}
