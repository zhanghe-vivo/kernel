use super::sys_config::{APBP_CLOCK, PL011_UART0_BASE, PL011_UART0_IRQ};
use crate::{
    arch::{
        interrupt::{IrqHandler, IrqNumber, IrqTrigger},
        Arch,
    },
    devices::{
        serial::{
            arm_pl011::{Interrupts, Uart, ALL_INTERRUPTS},
            config::{SerialConfig, _19200_8_N_1},
            Serial, SerialError, UartOps,
        },
        DeviceManager, DeviceRequest,
    },
    irq::Irq,
    sync::lock::spinlock::SpinLock,
};
use alloc::{boxed::Box, string::String, sync::Arc};
use core::ptr::NonNull;
use embedded_io::{ErrorKind, ErrorType, Read, ReadReady, Write, WriteReady};
use safe_mmio::UniqueMmioPointer;
use spin::Once;

struct UartDriver {
    inner: Uart<'static>,
    irq: IrqNumber,
}

impl ErrorType for UartDriver {
    type Error = SerialError;
}

impl Write for UartDriver {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
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

    fn flush(&mut self) -> Result<(), Self::Error> {
        while self.inner.is_busy() {}
        Ok(())
    }
}

impl WriteReady for UartDriver {
    fn write_ready(&mut self) -> Result<bool, SerialError> {
        Ok(!self.inner.is_tx_fifo_full())
    }
}

impl Read for UartDriver {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, SerialError> {
        if buf.is_empty() {
            return Ok(0);
        }

        let mut count = 0;
        while count < buf.len() {
            match self.inner.read_word() {
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
        Ok(!self.inner.is_rx_fifo_empty())
    }
}

impl UartDriver {
    fn new(base: u64, irq: IrqNumber) -> Self {
        let uart_address = unsafe { UniqueMmioPointer::new(NonNull::new(base as *mut _).unwrap()) };
        let mut inner = Uart::new(uart_address);
        let _ = inner.enable(&_19200_8_N_1, APBP_CLOCK);
        Self { inner, irq }
    }
}

impl Drop for UartDriver {
    fn drop(&mut self) {
        self.inner.disable();
    }
}

impl UartOps for UartDriver {
    fn setup(&mut self, serial_config: &SerialConfig) -> Result<(), SerialError> {
        let uart = &mut self.inner;
        let _ = uart.enable(serial_config, APBP_CLOCK);
        uart.clear_interrupts(ALL_INTERRUPTS);
        Arch::enable_irq(self.irq, 0);
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), SerialError> {
        Arch::disable_irq(self.irq, 0);
        self.inner.disable();
        Ok(())
    }

    fn read_byte(&mut self) -> Result<u8, SerialError> {
        match self.inner.read_word()? {
            Some(byte) => Ok(byte),
            None => Err(SerialError::BufferEmpty),
        }
    }

    fn write_byte(&mut self, byte: u8) -> Result<(), SerialError> {
        self.inner.write_word(byte);
        Ok(())
    }

    fn write_str(&mut self, s: &str) -> Result<(), SerialError> {
        for c in s.as_bytes() {
            while self.inner.is_tx_fifo_full() {}
            self.inner.write_word(*c);
        }
        Ok(())
    }

    fn set_rx_interrupt(&mut self, enable: bool) {
        let mut masks = self.inner.interrupt_masks();
        if enable {
            masks |= Interrupts::RXI;
        } else {
            masks &= !Interrupts::RXI;
        }
        self.inner.set_interrupt_masks(masks);
    }

    fn set_tx_interrupt(&mut self, enable: bool) {
        let mut masks = self.inner.interrupt_masks();
        if enable {
            masks |= Interrupts::TXI;
        } else {
            masks &= !Interrupts::TXI;
        }
        self.inner.set_interrupt_masks(masks);
    }

    fn clear_rx_interrupt(&mut self) {
        self.inner.clear_interrupts(Interrupts::RXI);
    }

    fn clear_tx_interrupt(&mut self) {
        self.inner.clear_interrupts(Interrupts::TXI);
    }

    fn ioctl(&mut self, request: u32, arg: usize) -> Result<(), SerialError> {
        match DeviceRequest::from(request) {
            DeviceRequest::Config => {
                let config = unsafe { *(arg as *const SerialConfig) };
                let _ = self.inner.enable(&config, APBP_CLOCK);
            }
            DeviceRequest::Close => {
                self.inner.disable();
            }
            _ => return Err(SerialError::InvalidParameter),
        }
        Ok(())
    }
}

static UART0: Once<SpinLock<UartDriver>> = Once::new();
static SERIAL0: Once<Arc<Serial>> = Once::new();

pub fn get_early_uart() -> &'static SpinLock<dyn UartOps> {
    UART0.call_once(|| SpinLock::new(UartDriver::new(PL011_UART0_BASE, PL011_UART0_IRQ)))
}

pub fn get_serial0() -> &'static Arc<Serial> {
    SERIAL0.call_once(|| {
        Arc::new(Serial::new(
            0,
            SerialConfig::default(),
            Arc::new(SpinLock::new(UartDriver::new(
                PL011_UART0_BASE,
                PL011_UART0_IRQ,
            ))),
        ))
    })
}

pub struct Serial0Irq {}
impl IrqHandler for Serial0Irq {
    fn handle(&mut self) {
        Irq::enter(PL011_UART0_IRQ);
        let serial0 = get_serial0();
        let _ = serial0.recvchars();
        serial0.uart_ops.lock().clear_rx_interrupt();

        let _ = serial0.xmitchars();
        serial0.uart_ops.lock().clear_tx_interrupt();
        Irq::leave();
    }
}

pub fn uart_init() -> Result<(), ErrorKind> {
    let serial0 = get_serial0();
    Arch::set_trigger(PL011_UART0_IRQ, 0, IrqTrigger::Level);
    let _ = Arch::register_handler(PL011_UART0_IRQ, Box::new(Serial0Irq {}));
    DeviceManager::get().register_device(String::from("ttyS0"), serial0.clone())
}
