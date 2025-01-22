use core::fmt;
use core::hint::spin_loop;
use embedded_io::{ErrorKind, ErrorType, Read, ReadReady, Write, WriteReady};

use tock_registers::{
    interfaces::{Readable, Writeable, ReadWriteable},
    register_bitfields, register_structs,
    registers::{ReadOnly, ReadWrite, WriteOnly},
};

register_bitfields! [
    u32,
    /// Data Register flags, see https://developer.arm.com/documentation/ddi0183/g/programmers-model/register-descriptions/data-register--uartdr
    Data [
        /// Data character
        DATA OFFSET(0) NUMBITS(8) [],
        /// Framing error
        FE OFFSET(8) NUMBITS(1) [],
        /// Parity error  
        PE OFFSET(9) NUMBITS(1) [],
        /// Break error
        BE OFFSET(10) NUMBITS(1) [],
        /// Overrun error
        OE OFFSET(11) NUMBITS(1) []
    ],

    /// Receive Status Register, see https://developer.arm.com/documentation/ddi0183/g/programmers-model/register-descriptions/receive-status-register---error-clear-register--uartrsr-uartecr
    ReceiveStatus [
        /// Framing error
        FE OFFSET(0) NUMBITS(1) [],
        /// Parity error  
        PE OFFSET(1) NUMBITS(1) [],
        /// Break error
        BE OFFSET(2) NUMBITS(1) [],
        /// Overrun error
        OE OFFSET(3) NUMBITS(1) []
    ],

    /// Flag Register flags, see https://developer.arm.com/documentation/ddi0183/g/programmers-model/register-descriptions/flag-register--uartfr
    Flags [
        /// Clear to send
        CTS OFFSET(0) NUMBITS(1) [],
        /// Data set ready
        DSR OFFSET(1) NUMBITS(1) [],
        /// Data carrier detect
        DCD OFFSET(2) NUMBITS(1) [],
        /// UART busy
        BUSY OFFSET(3) NUMBITS(1) [],
        /// Receive FIFO empty
        RXFE OFFSET(4) NUMBITS(1) [],
        /// Transmit FIFO full
        TXFF OFFSET(5) NUMBITS(1) [],
        /// Receive FIFO full
        RXFF OFFSET(6) NUMBITS(1) [],
        /// Transmit FIFO empty
        TXFE OFFSET(7) NUMBITS(1) [],
        /// Ring indicator
        RI OFFSET(8) NUMBITS(1) []
    ],

    LineControl [
        /// Send Break
        BRK OFFSET(0) NUMBITS(1) [],
        /// Parity Enable
        PEN OFFSET(1) NUMBITS(1) [
            Disable = 0,
            Enable = 1,
        ],
        /// Even Parity Select
        EPS OFFSET(2) NUMBITS(1) [
            Odd = 0,
            Even = 1,
        ],
        /// Two stop bits select. If this bit is set to 1, two stop bits are transmitted at the end of the frame. The receive logic does not check for two stop bits being received.
        STP2 OFFSET(3) NUMBITS(1) [],
        /// Enable FIFOs
        FEN OFFSET(4) NUMBITS(1) [
            Disable = 0, // character mode
            Enable = 1,
        ],
        /// Word Length
        WLEN OFFSET(5) NUMBITS(2) [
            FiveBit = 0b00,
            SixBit = 0b01,
            SevenBit = 0b10,
            EightBit = 0b11
        ],
        /// Stick Parity Select
        SPS OFFSET(7) NUMBITS(1)[],
    ],
    /// Control Register flags
    Control [
        /// UART Enable
        UARTEN OFFSET(0) NUMBITS(1) [
            Disabled = 0,
            Enabled = 1
        ],
        /// SIR Enable
        SIREN OFFSET(1) NUMBITS(1) [],
        /// SIR Low-power Mode
        SIRLP OFFSET(2) NUMBITS(1) [],
        /// Loopback Enable
        LBE OFFSET(7) NUMBITS(1) [],
        /// Transmit Enable
        TXE OFFSET(8) NUMBITS(1) [
            Disabled = 0,
            Enabled = 1
        ],
        /// Receive Enable
        RXE OFFSET(9) NUMBITS(1) [
            Disabled = 0,
            Enabled = 1
        ],
        /// Data Transmit Ready
        DTR OFFSET(10) NUMBITS(1) [],
        /// Request to Send
        RTS OFFSET(11) NUMBITS(1) [],
        /// Out1
        OUT1 OFFSET(12) NUMBITS(1) [],
        /// Out2
        OUT2 OFFSET(13) NUMBITS(1) [],
        /// RTS Hardware Flow Control
        RTSEN OFFSET(14) NUMBITS(1) [],
        /// CTS Hardware Flow Control
        CTSEN OFFSET(15) NUMBITS(1) []
    ],
    InterruptFifoLevelSelect [
        /// Transmit interrupt FIFO level select
        TXIFLSEL OFFSET(0) NUMBITS(2) [],
        /// Receive Interrupt FIFO Level Select
        RXIFLSEL OFFSET(3) NUMBITS(2) [],
    ],
];

/// PL011 UART Registers, see https://developer.arm.com/documentation/ddi0183/g/programmers-model/summary-of-registers?lang=en
register_structs! {
    #[allow(non_snake_case)]
    Registers {
        /// Data Register
        (0x000 => DR: ReadWrite<u32, Data::Register>),
        /// Receive Status Register / Error Clear Register
        (0x004 => RSR: ReadWrite<u32, ReceiveStatus::Register>),
        (0x008 => _reserved1),
        (0x00C => _reserved2),
        (0x010 => _reserved3),
        (0x014 => _reserved4),
        /// Flag Register
        (0x018 => FR: ReadOnly<u32, Flags::Register>),
        (0x01C => _reserved5),
        /// IrDA Low-Power Counter Register,
        (0x020 => ILPR: ReadWrite<u32>),
        /// Integer Baud Rate Register
        (0x024 => IBRD: ReadWrite<u32>),
        /// Fractional Baud Rate Register
        (0x028 => FBRD: ReadWrite<u32>),
        /// Line Control Register
        (0x02C => LCR_H: ReadWrite<u32, LineControl::Register>),
        /// Control Register
        (0x030 => CR: ReadWrite<u32, Control::Register>),
        /// Interrupt FIFO Level Select Register
        (0x034 => IFLS: ReadWrite<u32, InterruptFifoLevelSelect::Register>),
        /// Interrupt Mask Set/Clear Register
        (0x038 => IMSC: ReadWrite<u32>),
        /// Raw Interrupt Status Register
        (0x03C => RIS: ReadOnly<u32>),
        /// Masked Interrupt Status Register
        (0x040 => MIS: ReadOnly<u32>),
        /// Interrupt Clear Register
        (0x044 => ICR: WriteOnly<u32>),
        /// DMA Control Register
        (0x048 => DMACR: ReadWrite<u32>),
        (0x04C => @END),
    }
}

/// Errors which may occur reading from a PL011 UART.
#[derive(Debug, Copy, Clone, Eq, PartialEq, thiserror::Error)]
pub enum Error {
    /// Break condition detected.
    #[error("Break condition detected")]
    Break,
    /// The received character did not have a valid stop bit.
    #[error("Framing error, received character didn't have a valid stop bit")]
    Framing,
    /// Data was received while the FIFO was already full.
    #[error("Overrun, data received while the FIFO was already full")]
    Overrun,
    /// Parity of the received data character did not match the selected parity.
    #[error("Parity of the received data character did not match the selected parity")]
    Parity,
}

impl embedded_io::Error for Error {
    fn kind(&self) -> ErrorKind {
        match self {
            Self::Break | Self::Overrun => ErrorKind::Other,
            Self::Framing | Self::Parity => ErrorKind::InvalidData,
        }
    }
}

/// Driver for a PL011 UART.
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
    pub unsafe fn new(base_address: *mut u32) -> Self {
        Self {
            registers: base_address as *mut Registers,
        }
    }

    /// Initializes PL011 UART.
    ///
    /// clock: Uart clock in Hz.
    /// baud_rate: Baud rate.
    pub fn init(&mut self, clock: u32, baud_rate: u32) {
        let divisor = (clock << 2) / baud_rate;

        // Disable UART before programming
        self.registers().CR.modify(Control::UARTEN::CLEAR);
                    
        // Program Integer Baud Rate
        self.registers().IBRD.set((divisor >> 6) as u32);

        // Program Fractional Baud Rate
        self.registers().FBRD.set((divisor & 0x3F) as u32);

        // Clear any pending errors
        self.registers().RSR.set(0);

        // Enable UART with RX and TX
        self.registers().CR.modify(
            Control::UARTEN::SET 
            + Control::TXE::SET 
            + Control::RXE::SET
        );
    }

    /// Writes a single byte to the UART.
    ///
    /// This blocks until there is space in the transmit FIFO or holding register, but returns as
    /// soon as the byte has been written to the transmit FIFO or holding register. It doesn't wait
    /// for the byte to be sent.
    pub fn write_byte(&mut self, byte: u8) {
        // Wait until there is room in the TX buffer.
        while self.registers().FR.is_set(Flags::TXFF) {
            spin_loop();
        }

        // Write to the TX buffer.
        self.registers().DR.set(byte as u32);
    }

    /// Reads and returns a pending byte, or `None` if nothing has been
    /// received.
    pub fn read_byte(&mut self) -> Result<Option<u8>, Error> {
        if self.registers().FR.is_set(Flags::RXFE) {
            Ok(None)
        } else {
            let data = self.registers().DR.extract();
            if data.is_set(Data::FE) {
                return Err(Error::Framing);
            }
            if data.is_set(Data::PE) {
                return Err(Error::Parity);
            }
            if data.is_set(Data::BE) {
                return Err(Error::Break);
            }
            if data.is_set(Data::OE) {
                return Err(Error::Overrun);
            }
            
            let byte = data.read(Data::DATA) as u8;
            Ok(Some(byte))
        }
    }

    /// Returns whether the UART is currently transmitting data.
    ///
    /// This will be true immediately after calling [`write_byte`](Self::write_byte).
    pub fn is_transmitting(&self) -> bool {
        self.registers().FR.is_set(Flags::BUSY)
    }

    #[inline]
    fn registers(&self) -> &Registers {
        // SAFETY: self.registers points to the control registers of a PL011 device which is
        // appropriately mapped, as promised by the caller of `Uart::new`.
        unsafe { &(*self.registers) }
    }
}

impl fmt::Write for Uart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.as_bytes() {
            self.write_byte(*c);
        }
        Ok(())
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
        Ok(!self.registers().FR.is_set(Flags::TXFF))
    }
}

impl Read for Uart {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        if buf.is_empty() {
            return Ok(0);
        }

        loop {
            if let Some(byte) = self.read_byte()? {
                buf[0] = byte;
                return Ok(1);
            }
        }
    }
}

impl ReadReady for Uart {
    fn read_ready(&mut self) -> Result<bool, Self::Error> {
        Ok(!self.registers().FR.is_set(Flags::RXFE))
    }
}