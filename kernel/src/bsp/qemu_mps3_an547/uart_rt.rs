use super::{
    cmsdk_uart::Uart,
    irq::{UARTRX0_IRQn, UARTRX1_IRQn},
    sys_config::{UART0_BASE_S, UART0_CLOCK, UART0_NAME, UART1_BASE_S, UART1_NAME},
};
use crate::{
    arch::{Arch, IrqNumber},
    kernel::irq::Irq,
    os_bindings,
};
use core::{ffi::c_char, mem::MaybeUninit};
use embedded_io::{Read, Write};

#[derive(Debug, Clone, Copy)]
enum UartIndex {
    #[cfg(feature = "enable_uart0")]
    Uart0,
    #[cfg(feature = "enable_uart1")]
    Uart1,
}

#[derive(Debug)]
#[repr(C)]
pub struct UartInstance {
    name: *const c_char,
    uart: Uart,
    irq: IrqNumber,
    index: UartIndex,
    serial: MaybeUninit<os_bindings::rt_serial_device>,
}

#[cfg(feature = "enable_uart0")]
pub static mut UART0_INSTANCE: UartInstance = UartInstance {
    name: UART0_NAME.as_ptr(),
    // Safety: Uart Base Address is valid
    uart: unsafe { Uart::new(UART0_BASE_S as *mut u32) },
    irq: UARTRX0_IRQn,
    index: UartIndex::Uart0,
    serial: MaybeUninit::uninit(),
};

#[cfg(feature = "enable_uart1")]
pub static mut UART1_INSTANCE: UartInstance = UartInstance {
    name: UART1_NAME.as_ptr(),
    // Safety: Uart Base Address is valid
    uart: unsafe { Uart::new(UART1_BASE_S as *mut u32) },
    irq: UARTRX1_IRQn,
    index: UartIndex::Uart1,
    serial: MaybeUninit::uninit(),
};

unsafe fn uart_isr(serial: *mut os_bindings::rt_serial_device) {
    os_bindings::rt_hw_serial_isr(serial, os_bindings::RT_SERIAL_EVENT_RX_IND as i32);
}

#[cfg(feature = "enable_uart0")]
#[link_section = ".text.vector_handlers"]
#[no_mangle]
pub unsafe extern "C" fn UARTRX0_Handler() {
    Irq::enter();
    uart_isr(UART0_INSTANCE.serial.assume_init_mut());
    UART0_INSTANCE.uart.clear_interrupt();
    Irq::leave();
}

#[cfg(feature = "enable_uart1")]
#[link_section = ".text.vector_handlers"]
#[no_mangle]
pub unsafe extern "C" fn UARTRX1_Handler() {
    Irq::enter();
    uart_isr(UART1_INSTANCE.serial.assume_init_mut());
    UART1_INSTANCE.uart.clear_interrupt();
    Irq::leave();
}

unsafe extern "C" fn uart_configure(
    serial: *mut os_bindings::rt_serial_device,
    cfg: *mut os_bindings::serial_configure,
) -> os_bindings::rt_err_t {
    let instance = &mut *((*serial).parent.user_data as *mut UartInstance);

    instance.uart.init(UART0_CLOCK, (*cfg).baud_rate);
    Arch::enable_irq(instance.irq);
    instance.uart.clear_interrupt();

    os_bindings::RT_EOK as i32
}

unsafe extern "C" fn uart_control(
    serial: *mut os_bindings::rt_serial_device,
    cmd: i32,
    _arg: *mut core::ffi::c_void,
) -> os_bindings::rt_err_t {
    let instance = &mut *((*serial).parent.user_data as *mut UartInstance);

    match cmd as u32 {
        os_bindings::RT_DEVICE_CTRL_CLR_INT => {
            instance.uart.disable_rx_interrupt();
        }
        os_bindings::RT_DEVICE_CTRL_SET_INT => {
            instance.uart.enable_rx_interrupt();
        }
        _ => {}
    }

    os_bindings::RT_EOK as i32
}

pub unsafe extern "C" fn uart_putc(serial: *mut os_bindings::rt_serial_device, c: i8) -> i32 {
    let instance = &mut *((*serial).parent.user_data as *mut UartInstance);
    match instance.uart.write(&[c as u8]) {
        Ok(1) => 1,
        _ => -1,
    }
}

pub unsafe extern "C" fn uart_getc(serial: *mut os_bindings::rt_serial_device) -> i32 {
    let instance = &mut *((*serial).parent.user_data as *mut UartInstance);

    let mut buf = [0u8; 1];
    match instance.uart.read(&mut buf) {
        Ok(1) => buf[0] as i32,
        _ => -1,
    }
}

static UART_OPS: os_bindings::rt_uart_ops = os_bindings::rt_uart_ops {
    configure: Some(uart_configure),
    control: Some(uart_control),
    putc: Some(uart_putc),
    getc: Some(uart_getc),
    dma_transmit: None,
};

#[repr(transparent)]
struct SerialConfigWrapper(os_bindings::serial_configure);

impl SerialConfigWrapper {
    pub fn new() -> Self {
        Self(os_bindings::serial_configure {
            baud_rate: os_bindings::BAUD_RATE_115200,
            _bitfield_align_1: [],
            _bitfield_1: os_bindings::serial_configure::new_bitfield_1(
                os_bindings::DATA_BITS_8,
                os_bindings::STOP_BITS_1,
                os_bindings::PARITY_NONE,
                os_bindings::BIT_ORDER_LSB,
                os_bindings::NRZ_NORMAL,
                os_bindings::RT_SERIAL_RB_BUFSZ,
                os_bindings::RT_SERIAL_FLOWCONTROL_NONE,
                0, // reserved
            ),
        })
    }
}

pub fn uart_init() {
    let config = SerialConfigWrapper::new();

    #[cfg(feature = "enable_uart0")]
    unsafe {
        let serial = UART0_INSTANCE.serial.assume_init_mut();
        serial.ops = &UART_OPS;
        serial.config = config.0;

        os_bindings::rt_hw_serial_register(
            serial as *mut _,
            UART0_INSTANCE.name,
            os_bindings::RT_DEVICE_FLAG_RDWR | os_bindings::RT_DEVICE_FLAG_INT_RX,
            &raw mut UART0_INSTANCE as *mut _ as *mut core::ffi::c_void,
        );
    }

    #[cfg(feature = "enable_uart1")]
    unsafe {
        let serial = UART1_INSTANCE.serial.assume_init_mut();
        serial.ops = &UART_OPS;
        serial.config = config.0;

        os_bindings::rt_hw_serial_register(
            serial as *mut _,
            UART1_INSTANCE.name,
            os_bindings::RT_DEVICE_FLAG_RDWR | os_bindings::RT_DEVICE_FLAG_INT_RX,
            &raw mut UART1_INSTANCE as *mut _ as *mut core::ffi::c_void,
        );
    }
}
