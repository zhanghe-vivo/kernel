use super::sys_config::{APBP_CLOCK, PL011_UART0_BASE};
use crate::{
    arch::aarch64::registers::cntfrq_el0::CNTFRQ_EL0,
    devices::{
        serial::{
            arm_pl011::{Interrupts, Uart, ALL_INTERRUPTS},
            config::{SerialConfig, _19200_8_N_1},
            Serial, SerialError, UartOps,
        },
        DeviceManager, DeviceRequest,
    },
    sync::SpinLock,
    vfs::AccessMode,
};
use alloc::{boxed::Box, sync::Arc};
use core::ptr::NonNull;
use embedded_io::{ErrorKind, ErrorType, Read, ReadReady, Write, WriteReady};
use safe_mmio::UniqueMmioPointer;
use spin::Once;

struct UartDriver {
    inner: Uart<'static>,
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
    fn new(base: u64) -> Self {
        let uart_address = unsafe { UniqueMmioPointer::new(NonNull::new(base as *mut _).unwrap()) };
        let mut inner = Uart::new(uart_address);
        inner.enable(&_19200_8_N_1, APBP_CLOCK);
        Self { inner }
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
        uart.enable(serial_config, APBP_CLOCK);
        uart.clear_interrupts(ALL_INTERRUPTS);
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), SerialError> {
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
                self.inner.enable(&config, APBP_CLOCK);
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
    UART0.call_once(|| SpinLock::new(UartDriver::new(PL011_UART0_BASE)))
}

pub fn get_serial0() -> &'static Arc<Serial> {
    SERIAL0.call_once(|| {
        Arc::new(Serial::new(
            0,
            SerialConfig::default(),
            Arc::new(SpinLock::new(UartDriver::new(PL011_UART0_BASE))),
        ))
    })
}
