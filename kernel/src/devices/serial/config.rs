//! Useful UART types

/// Data bits
#[derive(Debug, Clone, Copy)]
pub enum DataBits {
    /// 5 bits
    Five,
    /// 6 bits
    Six,
    /// 7 bits
    Seven,
    /// 8 bits
    Eight,
}

/// Stop bits
#[derive(Debug, Clone, Copy)]
pub enum StopBits {
    /// 1 bit
    One,
    /// 2 bits
    Two,
}

/// Parity
#[derive(Debug, Clone, Copy)]
pub enum Parity {
    None,
    /// Odd parity
    Odd,
    /// Even parity
    Even,
    One,
    Zero,
}

/// A struct holding the configuration for an UART device.
#[derive(Debug, Clone, Copy)]
pub struct SerialConfig {
    /// The baudrate the uart will run at.
    pub baudrate: u32,

    /// The amount of data bits the uart should be configured to.
    pub data_bits: DataBits,

    /// The amount of stop bits the uart should be configured to.
    pub stop_bits: StopBits,

    /// The parity that this uart should have
    pub parity: Parity,
}

impl SerialConfig {
    /// Create a new instance of Uart SerialConfig
    pub const fn new(
        baudrate: u32,
        data_bits: DataBits,
        parity: Parity,
        stop_bits: StopBits,
    ) -> SerialConfig {
        SerialConfig {
            baudrate,
            data_bits,
            stop_bits,
            parity,
        }
    }
}

impl Default for SerialConfig {
    fn default() -> Self {
        Self {
            baudrate: 115200,
            data_bits: DataBits::Eight,
            stop_bits: StopBits::One,
            parity: Parity::None,
        }
    }
}

/// 9600 baud, 8 data bits, no parity, 1 stop bit
pub const _9600_8_N_1: SerialConfig = SerialConfig {
    baudrate: 9600,
    data_bits: DataBits::Eight,
    stop_bits: StopBits::One,
    parity: Parity::None,
};

/// 19200 baud, 8 data bits, no parity, 1 stop bit
pub const _19200_8_N_1: SerialConfig = SerialConfig {
    baudrate: 19200,
    data_bits: DataBits::Eight,
    stop_bits: StopBits::One,
    parity: Parity::None,
};

/// 38400 baud, 8 data bits, no parity, 1 stop bit
pub const _38400_8_N_1: SerialConfig = SerialConfig {
    baudrate: 38400,
    data_bits: DataBits::Eight,
    stop_bits: StopBits::One,
    parity: Parity::None,
};

/// 57600 baud, 8 data bits, no parity, 1 stop bit
pub const _57600_8_N_1: SerialConfig = SerialConfig {
    baudrate: 57600,
    data_bits: DataBits::Eight,
    stop_bits: StopBits::One,
    parity: Parity::None,
};

/// 115200 baud, 8 data bits, no parity, 1 stop bit
pub const _115200_8_N_1: SerialConfig = SerialConfig {
    baudrate: 115200,
    data_bits: DataBits::Eight,
    stop_bits: StopBits::One,
    parity: Parity::None,
};
