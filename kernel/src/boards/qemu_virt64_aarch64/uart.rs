use super::config::{APBP_CLOCK, PL011_UART0_BASE, PL011_UART0_IRQNUM};
use crate::{
    arch::{irq, irq::IrqHandler},
    devices::{
        tty::{
            serial::{arm_pl011::Uart, Serial, UartOps},
            termios::{Cflags, Iflags, Lflags, Oflags, Termios},
        },
        DeviceManager,
    },
    irq::IrqTrace,
    sync::SpinLock,
};
use alloc::{
    boxed::Box,
    string::{String, ToString},
    sync::Arc,
};
use core::ptr::NonNull;
use embedded_io::{ErrorKind, ErrorType, Read, ReadReady, Write, WriteReady};
use safe_mmio::UniqueMmioPointer;
use spin::Once;

static UART0: Once<SpinLock<Uart<'static>>> = Once::new();
pub fn get_early_uart() -> &'static SpinLock<dyn UartOps> {
    UART0.call_once(|| {
        let mut uart = unsafe {
            Uart::new(UniqueMmioPointer::new(
                NonNull::new(PL011_UART0_BASE as *mut _).unwrap(),
            ))
        };
        let termios = Termios::new(
            Iflags::default(),
            Oflags::default(),
            Cflags::default(),
            Lflags::default(),
            19200,
            19200,
        );
        uart.enable(&termios, APBP_CLOCK);
        SpinLock::new(uart)
    })
}

static SERIAL0: Once<Arc<Serial>> = Once::new();

pub fn get_serial0() -> &'static Arc<Serial> {
    SERIAL0.call_once(|| {
        let mut uart = unsafe {
            Uart::new(UniqueMmioPointer::new(
                NonNull::new(PL011_UART0_BASE as *mut _).unwrap(),
            ))
        };
        let termios = Termios::new(
            Iflags::default(),
            Oflags::default(),
            Cflags::default(),
            Lflags::default(),
            19200,
            19200,
        );
        uart.enable(&termios, APBP_CLOCK);
        Arc::new(Serial::new(0, termios, Arc::new(SpinLock::new(uart))))
    })
}

pub struct Serial0Irq {}
impl IrqHandler for Serial0Irq {
    fn handle(&mut self) {
        let _ = IrqTrace::new(PL011_UART0_IRQNUM);
        let serial0 = get_serial0();
        let _ = serial0.recvchars();
        serial0.uart_ops.lock().clear_rx_interrupt();

        let _ = serial0.xmitchars();
        serial0.uart_ops.lock().clear_tx_interrupt();
    }
}

pub fn uart_init() -> Result<(), ErrorKind> {
    let serial0 = get_serial0();
    irq::set_trigger(PL011_UART0_IRQNUM, 0, irq::IrqTrigger::Level);
    let _ = irq::register_handler(PL011_UART0_IRQNUM, Box::new(Serial0Irq {}));
    DeviceManager::get().register_device(String::from("ttyS0"), serial0.clone())
}
