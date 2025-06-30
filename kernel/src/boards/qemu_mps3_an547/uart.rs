use super::config::{memory_map::UART0_BASE_S, UART0RX_IRQn, UART0TX_IRQn, SYSTEM_CORE_CLOCK};
use crate::{
    devices::{
        tty::{
            serial::{cmsdk_uart::Uart, Serial, UartOps},
            termios::Termios,
        },
        DeviceManager,
    },
    irq::IrqTrace,
    sync::SpinLock,
};
use alloc::{string::String, sync::Arc};
use embedded_io::ErrorKind;
use spin::Once;

static UART0: Once<SpinLock<Uart>> = Once::new();
pub fn get_early_uart() -> &'static SpinLock<dyn UartOps> {
    UART0.call_once(|| {
        let mut uart = unsafe { Uart::new(UART0_BASE_S as *mut u32) };
        uart.enable(SYSTEM_CORE_CLOCK, 115200);
        SpinLock::new(uart)
    })
}

static SERIAL0: Once<Arc<Serial>> = Once::new();

pub fn get_serial0() -> &'static Arc<Serial> {
    SERIAL0.call_once(|| {
        let mut uart = unsafe { Uart::new(UART0_BASE_S as *mut u32) };
        uart.enable(SYSTEM_CORE_CLOCK, 115200);
        Arc::new(Serial::new(
            0,
            Termios::default(),
            Arc::new(SpinLock::new(uart)),
        ))
    })
}

pub fn uart_init() -> Result<(), ErrorKind> {
    let serial0 = get_serial0();
    DeviceManager::get().register_device(String::from("ttyS0"), serial0.clone())
}

#[coverage(off)]
pub unsafe extern "C" fn uartrx0_handler() {
    let _ = IrqTrace::new(UART0RX_IRQn);
    let uart = get_serial0();
    uart.uart_ops.irqsave_lock().clear_rx_interrupt();
    if let Err(_e) = uart.recvchars() {
        // println!("UART RX error: {:?}", e);
    }
}

#[coverage(off)]
pub unsafe extern "C" fn uarttx0_handler() {
    let _ = IrqTrace::new(UART0TX_IRQn);
    let uart = get_serial0();
    uart.uart_ops.irqsave_lock().clear_tx_interrupt();
    if let Err(_e) = uart.xmitchars() {
        // println!("UART TX error: {:?}", e);
    }
}
