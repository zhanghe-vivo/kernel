use super::sys_config::*;
use crate::{
    arch::{
        interrupt::{IrqHandler, IrqNumber},
        registers::cntfrq_el0::CNTFRQ_EL0,
        Arch,
    },
    drivers::serial::arm_pl011::*,
};
use alloc::boxed::Box;
use tock_registers::interfaces::{Readable, Writeable};

pub static mut UART0: Uart = unsafe { Uart::new(UART0_BASE_S as *mut u32) };

struct UartIrq {}

impl IrqHandler for UartIrq {
    fn handle(&mut self) -> Result<(), &'static str> {
        unsafe {
            UART0.registers().ICR.write(Icr::ALL::CLEAR);
            while !UART0.registers().FR.is_set(Flags::RXFE) {
                UART0.write_byte(UART0.registers().DR.get() as u8);
            }
        }
        Ok(())
    }
}

pub fn uart_init() {
    Arch::register_handler(IrqNumber(PL011_UART0_IRQNUM), Box::new(UartIrq {}));
    let frequency = CNTFRQ_EL0.get();
    unsafe { UART0.init(frequency as u32, 115200) };
}
