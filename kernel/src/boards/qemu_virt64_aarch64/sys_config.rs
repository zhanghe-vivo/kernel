use crate::arch::IrqNumber;

pub const DRAM_BASE: u64 = 0x4000_0000;
pub const TICK_PER_SECOND: u64 = 100;
pub const TIME_IRQ_NUM: IrqNumber = IrqNumber::new(30);
pub const APBP_CLOCK: u32 = 0x16e3600; // 24MHz
pub const PL011_UART0_BASE: u64 = 0x900_0000;
pub const PL011_UART0_IRQ: IrqNumber = IrqNumber::new(33);

pub const HEAP_SIZE: u64 = 16 * 1024 * 1024;
