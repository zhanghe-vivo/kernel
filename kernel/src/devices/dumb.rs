use crate::{
    devices::serial::{config::SerialConfig, Serial, SerialError, UartOps},
    sync::SpinLock,
    vfs::AccessMode,
};
use alloc::sync::Arc;
use embedded_io::{ErrorKind, ErrorType, Read, ReadReady, Write, WriteReady};
use spin::Once;

struct DumbUart;

pub(crate) static DUMB_UART0: SpinLock<DumbUart> = SpinLock::new(DumbUart);

unsafe impl Send for DumbUart {}
unsafe impl Sync for DumbUart {}

impl WriteReady for DumbUart {
    fn write_ready(&mut self) -> Result<bool, SerialError> {
        Ok(true)
    }
}

impl ReadReady for DumbUart {
    fn read_ready(&mut self) -> Result<bool, SerialError> {
        Ok(true)
    }
}

impl Read for DumbUart {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, SerialError> {
        Ok(buf.len())
    }
}

impl Write for DumbUart {
    fn write(&mut self, buf: &[u8]) -> Result<usize, SerialError> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<(), SerialError> {
        Ok(())
    }
}

impl ErrorType for DumbUart {
    type Error = SerialError;
}

impl UartOps for DumbUart {
    fn setup(&mut self, _: &SerialConfig) -> Result<(), SerialError> {
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), SerialError> {
        Ok(())
    }

    fn read_byte(&mut self) -> Result<u8, SerialError> {
        Ok(0u8)
    }

    fn write_byte(&mut self, c: u8) -> Result<(), SerialError> {
        Ok(())
    }

    fn write_str(&mut self, s: &str) -> Result<(), SerialError> {
        Ok(())
    }

    fn ioctl(&mut self, request: u32, arg: usize) -> Result<(), SerialError> {
        Ok(())
    }

    fn set_rx_interrupt(&mut self, enable: bool) {}

    fn set_tx_interrupt(&mut self, enable: bool) {}

    fn clear_rx_interrupt(&mut self) {}

    fn clear_tx_interrupt(&mut self) {}
}

pub(crate) fn get_early_uart<'a>() -> &'a SpinLock<dyn UartOps> {
    return &DUMB_UART0;
}

static DUMB_SERIAL0: Once<Arc<Serial>> = Once::new();

pub(crate) fn get_serial0() -> &'static Arc<Serial> {
    DUMB_SERIAL0.call_once(|| {
        let uart = Arc::new(SpinLock::new(DumbUart));
        Arc::new(Serial::new(0, SerialConfig::default(), uart))
    })
}
