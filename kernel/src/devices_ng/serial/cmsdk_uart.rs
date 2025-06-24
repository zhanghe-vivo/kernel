// SPDX-FileCopyrightText: Copyright 2023-2024 Arm Limited and/or its affiliates <open-source-office@arm.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::devices::serial::SerialError;
use core::hint::spin_loop;

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
        RXORIRQ OFFSET(3) NUMBITS(1) [], // write 1 to clear
        /// Transmit overrun interrupt
        TXORIRQ OFFSET(2) NUMBITS(1) [], // write 1 to clear
        /// Receive interrupt
        RXIRQ OFFSET(1) NUMBITS(1) [], // write 1 to clear
        /// Transmit interrupt
        TXIRQ OFFSET(0) NUMBITS(1) [] // write 1 to clear
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
    pub fn enable(&mut self, clock: u32, baud_rate: u32) {
        let divisor = (clock << 2) / baud_rate;

        self.registers()
            .CTRL
            .modify(CTRL::RXEN::SET + CTRL::TXEN::SET);
        self.registers().BAUDDIV.set(divisor as u32);
        self.registers().INTSTATUS.set(0xf);
    }

    pub fn disable(&mut self) {
        self.registers().CTRL.modify(
            CTRL::RXIRQEN::CLEAR + CTRL::RXEN::CLEAR + CTRL::TXIRQEN::CLEAR + CTRL::TXEN::CLEAR,
        );
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
    pub fn clear_rx_interrupt(&mut self) {
        self.registers().INTSTATUS.set(INTSTATUS::RXIRQ::SET.into());
    }

    #[inline]
    pub fn clear_tx_interrupt(&mut self) {
        self.registers().INTSTATUS.set(INTSTATUS::TXIRQ::SET.into());
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
    pub fn enable_tx_interrupt(&mut self) {
        self.registers().CTRL.modify(CTRL::TXIRQEN::SET);
    }

    #[inline]
    pub fn disable_tx_interrupt(&mut self) {
        self.registers().CTRL.modify(CTRL::TXIRQEN::CLEAR);
    }

    /// Reads and returns a pending byte, or `None` if nothing has been
    /// received.
    pub fn read_data(&mut self) -> Result<Option<u8>, SerialError> {
        let state = self.registers().STATE.extract();

        if !state.is_set(STATE::RXBF) {
            // no data
            Ok(None)
        } else if state.is_set(STATE::RXOR) {
            Err(SerialError::Overrun)
        } else {
            let ch = self.registers().DATA.read(DATA::DATA) as u8;
            self.registers().STATE.set(0);
            Ok(Some(ch))
        }
    }

    /// Writes a single byte to the UART.
    pub fn write_data(&mut self, byte: u8) {
        while self.registers().STATE.is_set(STATE::TXBF) {
            spin_loop();
        }

        self.registers().DATA.write(DATA::DATA.val(byte as u32));
    }

    pub fn try_write_data(&mut self, byte: u8) -> Result<(), SerialError> {
        if self.registers().STATE.is_set(STATE::TXBF) {
            Err(SerialError::Overrun)
        } else {
            self.write_data(byte);
            Ok(())
        }
    }

    pub fn is_rx_fifo_full(&self) -> bool {
        self.registers().STATE.is_set(STATE::RXBF)
    }

    pub fn is_tx_fifo_full(&self) -> bool {
        self.registers().STATE.is_set(STATE::TXBF)
    }

    #[inline]
    fn registers(&self) -> &Registers {
        // SAFETY: self.registers points to the control registers of a PL011 device which is
        // appropriately mapped, as promised by the caller of `Uart::new`.
        unsafe { &(*self.registers) }
    }
}

// SAFETY: `Uart` just contains a pointer to device memory, which can be accessed from any context.
// The pointer is guaranteed to be valid and properly aligned by the caller of `Uart::new`.
unsafe impl Send for Uart {}

// SAFETY: Methods on `&Uart` don't allow changing any state so are safe to call concurrently from
// any context. The pointer is guaranteed to be valid and properly aligned by the caller of `Uart::new`.
unsafe impl Sync for Uart {}
