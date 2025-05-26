use super::{
    device::{Device, DeviceManager},
    serial::{config::SerialConfig, Serial, UartOps},
};
use crate::{sync::SpinLock, vfs::vfs_mode::AccessMode};
use alloc::sync::Arc;
use embedded_io::ErrorKind;
use spin::Once;

static CONSOLE: Once<Arc<dyn Device>> = Once::new();

pub fn init_console(uart: Arc<SpinLock<dyn UartOps>>) -> Result<(), ErrorKind> {
    let console = CONSOLE.call_once(|| {
        Arc::new(Serial::new(
            "console",
            AccessMode::O_RDWR,
            SerialConfig::default(),
            uart,
        ))
    });
    DeviceManager::get().register_device("console", console.clone())
}

pub fn get_console() -> Arc<dyn Device> {
    CONSOLE.get().unwrap().clone()
}

pub fn get_early_uart() -> &'static Arc<SpinLock<dyn UartOps>> {
    crate::boards::uart::get_early_uart()
}
