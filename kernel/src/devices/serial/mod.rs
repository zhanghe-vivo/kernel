use crate::{
    clock::WAITING_FOREVER,
    cpu::Cpu,
    devices::{Device, DeviceBase, DeviceClass, DeviceId, DeviceRequest},
    sync::{
        futex::{atomic_wait, atomic_wake},
        lock::spinlock::SpinLock,
    },
};
use alloc::{format, string::String, sync::Arc};
use bluekernel_infra::ringbuffer::BoxedRingBuffer;
use bluekernel_kconfig::{SERIAL_RX_FIFO_SIZE, SERIAL_TX_FIFO_SIZE};
use core::sync::atomic::AtomicUsize;
use delegate::delegate;
use embedded_io::{ErrorKind, ErrorType, Read, ReadReady, Write, WriteReady};

pub mod arm_pl011;
pub mod cmsdk_uart;
pub mod config;

use config::SerialConfig;

const SERIAL_RX_FIFO_MIN_SIZE: usize = 256;
const SERIAL_TX_FIFO_MIN_SIZE: usize = 256;

#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
pub enum SerialError {
    #[error("Overrun")]
    Overrun,
    #[error("Break")]
    Break,
    #[error("Parity")]
    Parity,
    #[error("Framing")]
    Framing,
    #[error("Buffer is empty")]
    BufferEmpty,
    #[error("Device error")]
    DeviceError,
    #[error("Invalid configuration")]
    InvalidParameter,
    #[error("Operation timed out")]
    TimedOut,
}

impl embedded_io::Error for SerialError {
    fn kind(&self) -> ErrorKind {
        match self {
            Self::Break | Self::Overrun => ErrorKind::Other,
            Self::Framing | Self::Parity => ErrorKind::InvalidData,
            Self::BufferEmpty | Self::InvalidParameter => ErrorKind::InvalidInput,
            Self::DeviceError => ErrorKind::Other,
            Self::TimedOut => ErrorKind::TimedOut,
        }
    }
}

impl From<SerialError> for ErrorKind {
    fn from(error: SerialError) -> Self {
        match error {
            SerialError::Break | SerialError::Overrun => ErrorKind::Other,
            SerialError::Framing | SerialError::Parity => ErrorKind::InvalidData,
            SerialError::BufferEmpty | SerialError::InvalidParameter => ErrorKind::InvalidInput,
            SerialError::DeviceError => ErrorKind::Other,
            SerialError::TimedOut => ErrorKind::TimedOut,
        }
    }
}

// TODO: add DMA support
pub trait UartOps:
    Read + Write + ReadReady + WriteReady + ErrorType<Error = SerialError> + Send + Sync
{
    fn setup(&mut self, serial_config: &SerialConfig) -> Result<(), SerialError>;
    fn shutdown(&mut self) -> Result<(), SerialError>;
    fn read_byte(&mut self) -> Result<u8, SerialError>;
    fn write_byte(&mut self, byte: u8) -> Result<(), SerialError>;
    fn write_str(&mut self, s: &str) -> Result<(), SerialError>;
    fn ioctl(&mut self, request: u32, arg: usize) -> Result<(), SerialError>;
    fn set_rx_interrupt(&mut self, enable: bool);
    fn set_tx_interrupt(&mut self, enable: bool);
    fn clear_rx_interrupt(&mut self);
    fn clear_tx_interrupt(&mut self);
}

#[derive(Debug)]
struct SerialRxFifo {
    rb: BoxedRingBuffer,
    futex: AtomicUsize,
}

#[derive(Debug)]
struct SerialTxFifo {
    rb: BoxedRingBuffer,
    futex: AtomicUsize,
}

impl SerialRxFifo {
    fn new(size: usize) -> Self {
        Self {
            rb: BoxedRingBuffer::new(size),
            futex: AtomicUsize::new(0),
        }
    }
}

impl SerialTxFifo {
    fn new(size: usize) -> Self {
        Self {
            rb: BoxedRingBuffer::new(size),
            futex: AtomicUsize::new(0),
        }
    }
}

pub struct Serial {
    base: DeviceBase,
    index: u32,
    config: SerialConfig,
    rx_fifo: SerialRxFifo,
    tx_fifo: SerialTxFifo,
    pub uart_ops: Arc<SpinLock<dyn UartOps>>,
}

impl Serial {
    pub fn new(index: u32, config: SerialConfig, uart_ops: Arc<SpinLock<dyn UartOps>>) -> Self {
        Self {
            base: DeviceBase::new(),
            index,
            config,
            rx_fifo: SerialRxFifo::new(SERIAL_RX_FIFO_SIZE.max(SERIAL_RX_FIFO_MIN_SIZE)),
            tx_fifo: SerialTxFifo::new(SERIAL_TX_FIFO_SIZE.max(SERIAL_TX_FIFO_MIN_SIZE)),
            uart_ops,
        }
    }

    delegate! {
        to self.base {
            fn inc_open_count(&self) -> u32;
            fn dec_open_count(&self) -> u32;
            fn is_opened(&self) -> bool;
        }
    }

    fn rx_disable(&self) -> Result<(), SerialError> {
        let _ = atomic_wake(&self.rx_fifo.futex as *const AtomicUsize as usize, 1);
        self.uart_ops.lock_irqsave().set_rx_interrupt(false);
        Ok(())
    }

    fn tx_disable(&self) -> Result<(), SerialError> {
        let _ = atomic_wake(&self.tx_fifo.futex as *const AtomicUsize as usize, 1);
        self.uart_ops.lock_irqsave().set_tx_interrupt(false);
        // send all data in tx fifo
        self.xmitchars()?;
        Ok(())
    }

    fn fifo_rx(&self, buf: &mut [u8], is_nonblocking: bool) -> Result<usize, SerialError> {
        let len = buf.len();
        let mut count = 0;
        let mut reader = unsafe { self.rx_fifo.rb.reader() };

        loop {
            // read data from ringbuffer
            let slices = reader.pop_slices();
            let mut n = 0;
            for slice in slices {
                let slice_len = slice.len().min(len - count);
                buf[count..count + slice_len].copy_from_slice(&slice[..slice_len]);
                count += slice_len;
                n += slice_len;
            }
            reader.pop_done(n);

            if !is_nonblocking {
                // if the available data is less than the requested data, wait for data
                if n == 0 {
                    atomic_wait(
                        &self.rx_fifo.futex as *const AtomicUsize as usize,
                        0,
                        WAITING_FOREVER,
                    )
                    .map_err(|_| SerialError::TimedOut)?;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        Ok(count)
    }

    fn fifo_tx(&self, buf: &[u8], is_nonblocking: bool) -> Result<usize, SerialError> {
        let len = buf.len();
        let mut count = 0;
        let mut writer = unsafe { self.tx_fifo.rb.writer() };

        loop {
            // Get all slice for writing
            let slices = writer.push_slices();
            let mut n = 0;
            for slice in slices {
                if slice.len() == 0 {
                    continue;
                }
                let slice_len = slice.len().min(len - count);
                slice[..slice_len].copy_from_slice(&buf[count..count + slice_len]);
                count += slice_len;
                n += slice_len;
            }
            if n > 0 {
                writer.push_done(n);
                self.uart_ops.lock_irqsave().set_tx_interrupt(true);
                // write some data to uart to trigger interrupt
                if !Cpu::is_in_interrupt() {
                    let _ = self.xmitchars();
                }
            }

            if !is_nonblocking && !Cpu::is_in_interrupt() {
                if !writer.is_empty() {
                    // wait for data to be written
                    atomic_wait(
                        &self.tx_fifo.futex as *const AtomicUsize as usize,
                        0,
                        WAITING_FOREVER,
                    )
                    .map_err(|_| SerialError::TimedOut)?;
                    self.uart_ops.lock_irqsave().set_tx_interrupt(false);
                } else if count >= len {
                    break;
                }
            } else {
                break;
            }
        }

        Ok(count)
    }

    /// this Function is called from the UART interrupt handler
    /// when an interrupt is received indicating that there is more space in the
    /// transmit FIFO
    pub fn xmitchars(&self) -> Result<usize, SerialError> {
        let mut nbytes: usize = 0;
        {
            let mut uart_ops = self.uart_ops.lock_irqsave();
            // Safety: tx_fifo reader is only accessed in the UART interrupt handler
            let mut reader = unsafe { self.tx_fifo.rb.reader() };
            while !reader.is_empty() && uart_ops.write_ready()? {
                let buf = reader.pop_slice();
                match uart_ops.write(buf) {
                    Ok(sent) => {
                        nbytes += sent;
                        reader.pop_done(sent);
                    }
                    Err(e) => return Err(e),
                }
            }
            if reader.is_empty() {
                uart_ops.set_tx_interrupt(false);
            }
        }

        if nbytes > 0 {
            // TODO: add notify for poll/select
            let _ = atomic_wake(&self.tx_fifo.futex as *const AtomicUsize as usize, 1);
        }

        Ok(nbytes)
    }

    /// this Function is called from the UART interrupt handler
    /// when an interrupt is received indicating that there is more data in the
    /// receive FIFO
    pub fn recvchars(&self) -> Result<usize, SerialError> {
        let mut nbytes: usize = 0;
        {
            let mut uart_ops = self.uart_ops.lock_irqsave();
            // Safety: rx_fifo writer is only accessed in the UART interrupt handler
            let mut writer = unsafe { self.rx_fifo.rb.writer() };
            while !writer.is_full() && uart_ops.read_ready()? {
                let buf = writer.push_slice();
                match uart_ops.read(buf) {
                    Ok(n) => {
                        nbytes += n;
                        writer.push_done(n);
                    }
                    Err(e) => return Err(e),
                }
            }
        }

        // TODO: add notify for poll/select
        if nbytes > 0 {
            let _ = atomic_wake(&self.rx_fifo.futex as *const AtomicUsize as usize, 1);
        }

        Ok(nbytes)
    }
}

impl Device for Serial {
    fn name(&self) -> String {
        format!("ttyS{}", self.index)
    }

    fn class(&self) -> DeviceClass {
        DeviceClass::Char
    }

    fn id(&self) -> DeviceId {
        DeviceId {
            major: 4, // 4 is the minor number for /dev/tty
            minor: 64 + self.index,
        }
    }

    fn open(&self) -> Result<(), ErrorKind> {
        if !self.is_opened() {
            let mut uart_ops = self.uart_ops.lock_irqsave();
            uart_ops.setup(&self.config)?;
            uart_ops.set_rx_interrupt(true);
        }

        // Update device state
        self.inc_open_count();
        Ok(())
    }

    fn close(&self) -> Result<(), ErrorKind> {
        if !self.is_opened() {
            return Err(ErrorKind::NotFound);
        }

        if self.dec_open_count() == 0 {
            self.rx_disable()?;
            self.tx_disable()?;

            let mut uart_ops = self.uart_ops.lock_irqsave();
            uart_ops.ioctl(DeviceRequest::Close as u32, 0)?;
        }

        Ok(())
    }

    fn read(&self, _pos: usize, buf: &mut [u8], is_nonblocking: bool) -> Result<usize, ErrorKind> {
        self.fifo_rx(buf, is_nonblocking).map_err(|e| e.into())
    }

    fn write(&self, _pos: usize, buf: &[u8], is_nonblocking: bool) -> Result<usize, ErrorKind> {
        self.fifo_tx(buf, is_nonblocking).map_err(|e| e.into())
    }

    fn ioctl(&self, request: u32, arg: usize) -> Result<(), ErrorKind> {
        let mut uart_ops = self.uart_ops.lock_irqsave();
        uart_ops.ioctl(request, arg).map_err(|e| e.into())
    }
}
