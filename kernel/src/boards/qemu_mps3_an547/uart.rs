use super::{
    irq::{UARTRX0_IRQn, UARTTX0_IRQn},
    sys_config::{SYSTEM_CORE_CLOCK, UART0_BASE_S},
};
use crate::{
    arch::{Arch, IrqNumber},
    drivers::{
        device::{DeviceManager, DeviceRequest},
        serial::{cmsdk_uart::Uart, config::SerialConfig, Serial, SerialError, UartOps},
    },
    irq::Irq,
    sync::SpinLock,
    vfs::vfs_mode::AccessMode,
};
use alloc::sync::Arc;
use core::hint::spin_loop;
use embedded_io::{ErrorKind, ErrorType, Read, ReadReady, Write, WriteReady};
use spin::Once;

struct UartDriver {
    inner: Uart,
    rx_irq: IrqNumber,
    tx_irq: IrqNumber,
}

impl ErrorType for UartDriver {
    type Error = SerialError;
}

impl Write for UartDriver {
    // write will block until all the data is transmitted
    fn write(&mut self, buf: &[u8]) -> Result<usize, SerialError> {
        assert!(!buf.is_empty());
        let mut count = 0;
        // write until the buffer is full
        while count < buf.len() {
            match self.inner.try_write_data(buf[count]) {
                Ok(_) => count += 1,
                Err(_e) => break,
            }
        }
        Ok(count)
    }

    fn flush(&mut self) -> Result<(), SerialError> {
        while self.inner.is_transmitting() {
            spin_loop();
        }
        Ok(())
    }
}

impl WriteReady for UartDriver {
    fn write_ready(&mut self) -> Result<bool, SerialError> {
        Ok(self.inner.write_ready())
    }
}

impl Read for UartDriver {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, SerialError> {
        if buf.is_empty() {
            return Ok(0);
        }

        let mut count = 0;
        while count < buf.len() {
            match self.inner.read_data() {
                Ok(Some(byte)) => {
                    buf[count] = byte;
                    count += 1;
                }
                Ok(None) => break,
                Err(e) => return Err(e),
            }
        }

        return Ok(count);
    }
}

impl ReadReady for UartDriver {
    fn read_ready(&mut self) -> Result<bool, SerialError> {
        Ok(self.inner.read_ready())
    }
}

impl UartDriver {
    fn new(base: u32) -> Self {
        let mut inner = unsafe { Uart::new(base as *mut u32) };
        inner.init(SYSTEM_CORE_CLOCK, 115200);
        Self {
            inner,
            rx_irq: UARTRX0_IRQn,
            tx_irq: UARTTX0_IRQn,
        }
    }
}

impl Drop for UartDriver {
    fn drop(&mut self) {
        self.inner.deinit();
    }
}

impl UartOps for UartDriver {
    fn setup(&mut self, serial_config: &SerialConfig) -> Result<(), SerialError> {
        let uart = &mut self.inner;
        uart.init(SYSTEM_CORE_CLOCK, serial_config.baudrate);
        uart.clear_interrupt();
        Arch::enable_irq(self.rx_irq);
        Arch::enable_irq(self.tx_irq);
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), SerialError> {
        Arch::disable_irq(self.rx_irq);
        Arch::disable_irq(self.tx_irq);
        self.inner.deinit();
        Ok(())
    }

    fn read_byte(&mut self) -> Result<u8, SerialError> {
        match self.inner.read_data()? {
            Some(byte) => Ok(byte),
            None => Err(SerialError::BufferEmpty),
        }
    }

    fn write_byte(&mut self, byte: u8) -> Result<(), SerialError> {
        self.inner.write_data(byte);
        Ok(())
    }

    fn write_str(&mut self, s: &str) -> Result<(), SerialError> {
        for c in s.as_bytes() {
            self.inner.write_data(*c);
        }
        Ok(())
    }

    fn set_rx_interrupt(&mut self, enable: bool) {
        if enable {
            self.inner.enable_rx_interrupt();
        } else {
            self.inner.disable_rx_interrupt();
        }
    }

    fn set_tx_interrupt(&mut self, enable: bool) {
        if enable {
            self.inner.enable_tx_interrupt();
        } else {
            self.inner.disable_tx_interrupt();
        }
    }

    fn clear_rx_interrupt(&mut self) {
        self.inner.clear_rx_interrupt();
    }

    fn clear_tx_interrupt(&mut self) {
        self.inner.clear_tx_interrupt();
    }

    fn ioctl(&mut self, request: u32, arg: usize) -> Result<(), SerialError> {
        match DeviceRequest::from(request) {
            DeviceRequest::Config => {
                let config = unsafe { *(arg as *const SerialConfig) };
                self.inner.init(SYSTEM_CORE_CLOCK, config.baudrate);
            }
            DeviceRequest::Close => {
                self.inner.deinit();
            }
            _ => return Err(SerialError::InvalidConfig),
        }
        Ok(())
    }
}

static UART0: Once<Arc<SpinLock<dyn UartOps>>> = Once::new();
static SERIAL0: Once<Arc<Serial>> = Once::new();

pub fn get_uart0() -> &'static Arc<SpinLock<dyn UartOps>> {
    UART0.call_once(|| Arc::new(SpinLock::new(UartDriver::new(UART0_BASE_S))))
}

pub fn get_early_uart() -> &'static Arc<SpinLock<dyn UartOps>> {
    get_uart0()
}

pub fn get_serial0() -> &'static Arc<Serial> {
    SERIAL0.call_once(|| {
        let uart = get_uart0().clone();
        Arc::new(Serial::new(
            "ttyS0",
            AccessMode::O_RDWR,
            SerialConfig::default(),
            uart,
        ))
    })
}

pub fn uart_init() -> Result<(), ErrorKind> {
    let serial0 = get_serial0();
    DeviceManager::get().register_device("ttyS0", serial0.clone())
}

#[link_section = ".text.vector_handlers"]
#[coverage(off)]
#[no_mangle]
pub unsafe extern "C" fn UARTRX0_Handler() {
    Irq::enter();
    let uart = get_serial0();
    uart.uart_ops.lock_irqsave().clear_rx_interrupt();

    if let Err(_e) = uart.uart_recvchars() {
        // println!("UART RX error: {:?}", e);
    }
    Irq::leave();
}

#[link_section = ".text.vector_handlers"]
#[coverage(off)]
#[no_mangle]
pub unsafe extern "C" fn UARTTX0_Handler() {
    Irq::enter();
    let uart = get_serial0();
    uart.uart_ops.lock_irqsave().clear_tx_interrupt();

    if let Err(_e) = uart.uart_xmitchars() {
        // println!("UART TX error: {:?}", e);
    }
    Irq::leave();
}
