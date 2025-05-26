use super::config::SerialConfig;
use crate::{
    clock::WAITING_FOREVER,
    drivers::device::{Device, DeviceBase, DeviceClass, DeviceId, DeviceRequest},
    sync::{
        futex::{atomic_wait, atomic_wake},
        lock::spinlock::SpinLock,
    },
    vfs::vfs_mode::AccessMode,
};
use alloc::sync::Arc;
use bluekernel_infra::ringbuffer::BoxedRingBuffer;
use bluekernel_kconfig::{SERIAL_RX_FIFO_SIZE, SERIAL_TX_FIFO_SIZE};
use core::sync::atomic::AtomicUsize;
use delegate::delegate;
use embedded_io::{ErrorKind, ErrorType, Read, ReadReady, Write, WriteReady};
use libc::{EAGAIN, EINVAL, EIO, ENOSPC, ETIMEDOUT};

const SERIAL_RX_FIFO_MIN_SIZE: usize = 256;
const SERIAL_TX_FIFO_MIN_SIZE: usize = 256;

#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
pub enum SerialError {
    #[error("Buffer is full")]
    BufferFull,
    #[error("Buffer is empty")]
    BufferEmpty,
    #[error("Device error")]
    DeviceError,
    #[error("Invalid configuration")]
    InvalidConfig,
    #[error("Operation timed out")]
    TimedOut,
}

impl embedded_io::Error for SerialError {
    fn kind(&self) -> ErrorKind {
        match self {
            Self::BufferFull => ErrorKind::WriteZero,
            Self::BufferEmpty => ErrorKind::InvalidInput,
            Self::DeviceError => ErrorKind::Other,
            Self::InvalidConfig => ErrorKind::InvalidInput,
            Self::TimedOut => ErrorKind::TimedOut,
        }
    }
}

impl From<SerialError> for i32 {
    fn from(error: SerialError) -> Self {
        match error {
            SerialError::BufferFull => -ENOSPC,    // No space left on device
            SerialError::BufferEmpty => -EAGAIN,   // Resource temporarily unavailable
            SerialError::DeviceError => -EIO,      // Input/output error
            SerialError::InvalidConfig => -EINVAL, // Invalid argument
            SerialError::TimedOut => -ETIMEDOUT,   // Operation timed out
        }
    }
}

impl From<SerialError> for ErrorKind {
    fn from(error: SerialError) -> Self {
        match error {
            SerialError::BufferFull => ErrorKind::WriteZero,
            SerialError::BufferEmpty => ErrorKind::InvalidInput,
            SerialError::DeviceError => ErrorKind::Other,
            SerialError::InvalidConfig => ErrorKind::InvalidInput,
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
    config: SerialConfig,
    rx_fifo: SerialRxFifo,
    tx_fifo: SerialTxFifo,
    pub uart_ops: Arc<SpinLock<dyn UartOps>>,
}

impl Serial {
    pub fn new(
        name: &'static str,
        access_mode: AccessMode,
        config: SerialConfig,
        uart_ops: Arc<SpinLock<dyn UartOps>>,
    ) -> Self {
        Self {
            base: DeviceBase::new(name, DeviceClass::Char, access_mode),
            config,
            rx_fifo: SerialRxFifo::new(SERIAL_RX_FIFO_SIZE.max(SERIAL_RX_FIFO_MIN_SIZE)),
            tx_fifo: SerialTxFifo::new(SERIAL_TX_FIFO_SIZE.max(SERIAL_TX_FIFO_MIN_SIZE)),
            uart_ops,
        }
    }

    delegate! {
        to self.base {
            fn check_permission(&self, oflag: i32) -> Result<(), ErrorKind>;
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
        self.uart_xmitchars()?;
        Ok(())
    }

    fn fifo_rx(&self, buf: &mut [u8], is_blocking: bool) -> Result<usize, SerialError> {
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

            if is_blocking {
                // if the available data is less than the requested data, wait for data
                if count < len {
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

    fn fifo_tx(&self, buf: &[u8], is_blocking: bool) -> Result<usize, SerialError> {
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
                let _ = self.uart_xmitchars();
            }

            if is_blocking {
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
    pub fn uart_xmitchars(&self) -> Result<(), SerialError> {
        let mut nbytes = 0;
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

        Ok(())
    }

    /// this Function is called from the UART interrupt handler
    /// when an interrupt is received indicating that there is more data in the
    /// receive FIFO
    pub fn uart_recvchars(&self) -> Result<(), SerialError> {
        let mut nbytes = 0;

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

        Ok(())
    }
}

impl Device for Serial {
    delegate! {
        to self.base {
            fn name(&self) -> &'static str;
            fn class(&self) -> DeviceClass;
            fn access_mode(&self) -> AccessMode;
        }
    }

    fn id(&self) -> DeviceId {
        // TODO: add device id
        DeviceId {
            major: 4, // TTY major number
            minor: 0, // First UART device
        }
    }

    fn open(&self, oflag: i32) -> Result<(), ErrorKind> {
        // Check flags first
        self.check_permission(oflag)?;

        if !self.is_opened() {
            let mut uart_ops = self.uart_ops.lock_irqsave();
            uart_ops.setup(&self.config)?;
            //uart_ops.set_tx_interrupt(true);
            uart_ops.set_rx_interrupt(true);
        }

        // Update device state
        self.inc_open_count();
        Ok(())
    }

    fn close(&self) -> Result<(), ErrorKind> {
        if !self.is_opened() {
            return Ok(());
        }

        if self.dec_open_count() == 0 {
            self.rx_disable()?;
            self.tx_disable()?;

            let mut uart_ops = self.uart_ops.lock_irqsave();
            uart_ops.ioctl(DeviceRequest::Close as u32, 0)?;
        }

        Ok(())
    }

    fn read(&self, _pos: usize, buf: &mut [u8], is_blocking: bool) -> Result<usize, ErrorKind> {
        self.fifo_rx(buf, is_blocking).map_err(|e| e.into())
    }

    fn write(&self, _pos: usize, buf: &[u8], is_blocking: bool) -> Result<usize, ErrorKind> {
        self.fifo_tx(buf, is_blocking).map_err(|e| e.into())
    }

    fn ioctl(&self, request: u32, arg: usize) -> Result<(), ErrorKind> {
        let mut uart_ops = self.uart_ops.lock_irqsave();
        uart_ops.ioctl(request, arg).map_err(|e| e.into())
    }
}
