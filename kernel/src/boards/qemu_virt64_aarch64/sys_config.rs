use crate::arch::interrupt::IrqNumber;

pub const UART0_BASE_S: u64 = 0x59303000;
pub const TICK_PER_SECOND: u64 = 100;
pub const TIME_IRQ_NUM: IrqNumber = IrqNumber::new(30);
pub const PL011_UART0_IRQNUM: IrqNumber = IrqNumber::new(33);
pub const HEAP_SIZE: u64 = 16 * 1024 * 1024;
