use super::{
    serial::{Serial, UartOps},
    Device, DeviceManager,
};
use crate::sync::SpinLock;
use alloc::{string::String, sync::Arc};
use embedded_io::ErrorKind;
use spin::Once;

static CONSOLE: Once<Arc<dyn Device>> = Once::new();

pub fn init_console(serial: &Arc<Serial>) -> Result<(), ErrorKind> {
    CONSOLE.call_once(|| serial.clone());
    DeviceManager::get().register_device(String::from("console"), serial.clone())
}

pub fn get_console() -> Arc<dyn Device> {
    CONSOLE.get().unwrap().clone()
}

pub fn get_early_uart() -> &'static SpinLock<dyn UartOps> {
    crate::boards::get_early_uart()
}
