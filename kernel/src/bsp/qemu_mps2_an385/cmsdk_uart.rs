use core::{fmt, hint::spin_loop};
use embedded_io::{ErrorKind, ErrorType, Read, ReadReady, Write, WriteReady};

use tock_registers::{
    interfaces::{ReadWriteable, Readable, Writeable},
    register_bitfields, register_structs,
    registers::ReadWrite,
};

register_bitfields! [
    u32,

    /// Data Register
    pub DATA [
        /// Data value
        DATA OFFSET(0) NUMBITS(8) []
    ],

    /// Status Register
    pub STATE [
        /// Receive overrun
        RXOR OFFSET(3) NUMBITS(1) [],
        /// Transmit overrun
        TXOR OFFSET(2) NUMBITS(1) [],
        /// Receive buffer full
        RXBF OFFSET(1) NUMBITS(1) [],
        /// Transmit buffer full
        TXBF OFFSET(0) NUMBITS(1) []
    ],

    /// Control Register
    pub CTRL [
        /// High-speed test mode
        HSTM OFFSET(6) NUMBITS(1) [],
        /// Receive overrun interrupt enable
        RXORIRQEN OFFSET(5) NUMBITS(1) [],
        /// Transmit overrun interrupt enable
        TXORIRQEN OFFSET(4) NUMBITS(1) [],
        /// Receive interrupt enable
        RXIRQEN OFFSET(3) NUMBITS(1) [],
        /// Transmit interrupt enable
        TXIRQEN OFFSET(2) NUMBITS(1) [],
        /// Receive enable
        RXEN OFFSET(1) NUMBITS(1) [],
        /// Transmit enable
        TXEN OFFSET(0) NUMBITS(1) []
    ],

    /// Interrupt Status Register
    pub INTSTATUS [
        /// Receive overrun interrupt
        RXORIRQ OFFSET(3) NUMBITS(1) [],
        /// Transmit overrun interrupt
        TXORIRQ OFFSET(2) NUMBITS(1) [],
        /// Receive interrupt
        RXIRQ OFFSET(1) NUMBITS(1) [],
        /// Transmit interrupt
        TXIRQ OFFSET(0) NUMBITS(1) []
    ],

    /// Baudrate Divider Register
    pub BAUDDIV [
        /// Baudrate divider
        BAUDDIV OFFSET(0) NUMBITS(20) []
    ]
];

register_structs! {
    /// CMSDK UART Registers
    #[allow(non_snake_case)]
    Registers {
        /// Data Register
        (0x000 => DATA: ReadWrite<u32, DATA::Register>),
        /// Status Register
        (0x004 => STATE: ReadWrite<u32, STATE::Register>),
        /// Control Register
        (0x008 => CTRL: ReadWrite<u32, CTRL::Register>),
        /// Interrupt Status/Clear Register
        (0x00C => INTSTATUS: ReadWrite<u32, INTSTATUS::Register>),
        /// Baudrate Divider Register
        (0x010 => BAUDDIV: ReadWrite<u32, BAUDDIV::Register>),
        (0x014 => @END),
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, thiserror::Error)]
pub enum Error {
    /// Data was received while the FIFO was already full.
    #[error("Overrun, data received while the FIFO was already full")]
    Overrun,
}

impl embedded_io::Error for Error {
    fn kind(&self) -> ErrorKind {
        match self {
            Self::Overrun => ErrorKind::Other,
        }
    }
}

/// CMSDK UART peripheral
#[derive(Debug)]
pub struct Uart {
    registers: *mut Registers,
}

impl Uart {
    /// Constructs a new instance of the UART driver for a PL011 device at the
    /// given base address.
    ///
    /// # Safety
    ///
    /// The given base address must point to the 14 MMIO control registers of a
    /// PL011 device, which must be mapped into the address space of the process
    /// as device memory and not have any other aliases.
    pub const unsafe fn new(base_address: *mut u32) -> Self {
        Self {
            registers: base_address as *mut Registers,
        }
    }

    /// Initializes CMSDK UART.
    ///
    /// clock: Uart clock in Hz.
    /// baud_rate: Baud rate.
    pub fn init(&mut self, clock: u32, baud_rate: u32) {
        let divisor = (clock << 2) / baud_rate;

        self.registers()
            .CTRL
            .modify(CTRL::RXIRQEN::SET + CTRL::RXEN::SET + CTRL::TXEN::SET);
        self.registers().BAUDDIV.set(divisor as u32);
        self.registers().INTSTATUS.set(0);
    }

    /// Writes a single byte to the UART.
    pub fn write_byte(&mut self, byte: u8) {
        while self.registers().STATE.is_set(STATE::TXBF) {
            spin_loop();
        }

        self.registers().DATA.write(DATA::DATA.val(byte as u32));
    }

    /// Reads and returns a pending byte, or `None` if nothing has been
    /// received.
    pub fn read_byte(&mut self) -> Result<Option<u8>, Error> {
        let state = self.registers().STATE.extract();

        if !state.is_set(STATE::RXBF) {
            // no data
            Ok(None)
        } else if state.is_set(STATE::RXOR) {
            Err(Error::Overrun)
        } else {
            let ch = self.registers().DATA.read(DATA::DATA) as u8;
            self.registers().STATE.set(0);
            Ok(Some(ch))
        }
    }

    #[inline]
    pub fn is_transmitting(&self) -> bool {
        self.registers().STATE.is_set(STATE::TXBF)
    }

    #[inline]
    pub fn clear_interrupt(&mut self) {
        let status = self.registers().INTSTATUS.get();
        self.registers().INTSTATUS.set(status);
    }

    #[inline]
    pub fn enable_rx_interrupt(&mut self) {
        self.registers().CTRL.modify(CTRL::RXIRQEN::SET);
    }

    #[inline]
    pub fn disable_rx_interrupt(&mut self) {
        self.registers().CTRL.modify(CTRL::RXIRQEN::CLEAR);
    }

    #[inline]
    fn registers(&self) -> &Registers {
        // SAFETY: self.registers points to the control registers of a PL011 device which is
        // appropriately mapped, as promised by the caller of `Uart::new`.
        unsafe { &(*self.registers) }
    }
}

// SAFETY: `Uart` just contains a pointer to device memory, which can be accessed from any context.
unsafe impl Send for Uart {}

// SAFETY: Methods on `&Uart` don't allow changing any state so are safe to call concurrently from
// any context.
unsafe impl Sync for Uart {}

impl ErrorType for Uart {
    type Error = Error;
}

impl fmt::Write for Uart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.as_bytes() {
            self.write_byte(*c);
        }
        Ok(())
    }
}

impl Write for Uart {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            Ok(0)
        } else {
            self.write_byte(buf[0]);
            Ok(1)
        }
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        while self.is_transmitting() {
            spin_loop();
        }
        Ok(())
    }
}

impl WriteReady for Uart {
    fn write_ready(&mut self) -> Result<bool, Self::Error> {
        Ok(!self.registers().STATE.is_set(STATE::TXBF))
    }
}

impl Read for Uart {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            return Ok(0);
        }

        if let Some(byte) = self.read_byte()? {
            buf[0] = byte;
            return Ok(1);
        }

        Ok(0)
    }
}

impl ReadReady for Uart {
    fn read_ready(&mut self) -> Result<bool, Self::Error> {
        Ok(!self.registers().STATE.is_set(STATE::RXBF))
    }
}
