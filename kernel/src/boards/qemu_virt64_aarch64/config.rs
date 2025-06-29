use crate::arch::irq::IrqNumber;

pub const UART0_BASE_S: u64 = 0x59303000;
pub const APBP_CLOCK: u32 = 0x16e3600;
pub const PL011_UART0_BASE: u64 = 0x900_0000;
pub const PL011_UART0_IRQNUM: IrqNumber = IrqNumber::new(33);
pub const HEAP_SIZE: u64 = 16 * 1024 * 1024;

pub const GICD: usize = 0x8000000;
pub const GICR: usize = 0x80a0000;
