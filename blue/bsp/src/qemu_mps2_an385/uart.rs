use super::{
    cmsdk_uart::Uart,
    irq::{UART0RX_IRQn, UART1RX_IRQn},
    sys_config::{SYSTEM_CORE_CLOCK, UART0_BASE, UART0_NAME, UART1_BASE, UART1_NAME},
};
use crate::rt_bindings;
use crate::arch::{Arch, IrqNumber};
use crate::kernel::irq::Irq;
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
    serial: MaybeUninit<rt_bindings::rt_serial_device>,
}

#[cfg(feature = "enable_uart0")]
pub static mut UART0_INSTANCE: UartInstance = UartInstance {
    name: UART0_NAME.as_char_ptr(),
    // Safety: Uart Base Address is valid
    uart: unsafe { Uart::new(UART0_BASE as *mut u32) },
    irq: UART0RX_IRQn,
    index: UartIndex::Uart0,
    serial: MaybeUninit::uninit(),
};

#[cfg(feature = "enable_uart1")]
pub static mut UART1_INSTANCE: UartInstance = UartInstance {
    name: UART1_NAME.as_char_ptr(),
    // Safety: Uart Base Address is valid
    uart: unsafe { Uart::new(UART1_BASE as *mut u32) },
    irq: UART1RX_IRQn,
    index: UartIndex::Uart1,
    serial: MaybeUninit::uninit(),
};

unsafe fn uart_isr(serial: *mut rt_bindings::rt_serial_device) {
    rt_bindings::rt_hw_serial_isr(serial, rt_bindings::RT_SERIAL_EVENT_RX_IND as i32);
}

#[cfg(feature = "enable_uart0")]
#[link_section = ".text.vector_handlers"]
#[no_mangle]
pub unsafe extern "C" fn UART0RX_Handler() {
    Irq::enter();
    uart_isr(UART0_INSTANCE.serial.assume_init_mut());
    UART0_INSTANCE.uart.clear_interrupt();
    Irq::leave();
}

#[cfg(feature = "enable_uart1")]
#[link_section = ".text.vector_handlers"]
#[no_mangle]
pub unsafe extern "C" fn UART1RX_Handler() {
    Irq::enter();
    uart_isr(UART1_INSTANCE.serial.assume_init_mut());
    UART1_INSTANCE.uart.clear_interrupt();
    Irq::leave();
}

unsafe extern "C" fn uart_configure(
    serial: *mut rt_bindings::rt_serial_device,
    cfg: *mut rt_bindings::serial_configure,
) -> rt_bindings::rt_err_t {
    let instance = &mut *((*serial).parent.user_data as *mut UartInstance);

    instance.uart.init(SYSTEM_CORE_CLOCK, (*cfg).baud_rate);
    // 启用 NVIC 中断
    Arch::enable_irq(instance.irq);
    // 清除状态
    instance.uart.clear_interrupt();

    rt_bindings::RT_EOK as i32
}

unsafe extern "C" fn uart_control(
    serial: *mut rt_bindings::rt_serial_device,
    cmd: i32,
    _arg: *mut core::ffi::c_void,
) -> rt_bindings::rt_err_t {
    let instance = &mut *((*serial).parent.user_data as *mut UartInstance);

    match cmd as u32 {
        rt_bindings::RT_DEVICE_CTRL_CLR_INT => {
            instance.uart.disable_rx_interrupt();
        }
        rt_bindings::RT_DEVICE_CTRL_SET_INT => {
            instance.uart.enable_rx_interrupt();
        }
        _ => {}
    }

    rt_bindings::RT_EOK as i32
}

pub unsafe extern "C" fn uart_putc(serial: *mut rt_bindings::rt_serial_device, c: i8) -> i32 {
    let instance = &mut *((*serial).parent.user_data as *mut UartInstance);
    match instance.uart.write(&[c as u8]) {
        Ok(1) => 1,
        _ => -1,
    }
}

pub unsafe extern "C" fn uart_getc(serial: *mut rt_bindings::rt_serial_device) -> i32 {
    let instance = &mut *((*serial).parent.user_data as *mut UartInstance);

    let mut buf = [0u8; 1];
    match instance.uart.read(&mut buf) {
        Ok(1) => buf[0] as i32,
        _ => -1,
    }
}

static UART_OPS: rt_bindings::rt_uart_ops = rt_bindings::rt_uart_ops {
    configure: Some(uart_configure),
    control: Some(uart_control),
    putc: Some(uart_putc),
    getc: Some(uart_getc),
    dma_transmit: None,
};

impl Default for rt_bindings::serial_configure {
    fn default() -> Self {
        let config = Self {
            baud_rate: rt_bindings::BAUD_RATE_115200,
            _bitfield_align_1: [],
            _bitfield_1: rt_bindings::serial_configure::new_bitfield_1(
                rt_bindings::DATA_BITS_8,
                rt_bindings::STOP_BITS_1,
                rt_bindings::PARITY_NONE,
                rt_bindings::BIT_ORDER_LSB,
                rt_bindings::NRZ_NORMAL,
                rt_bindings::RT_SERIAL_RB_BUFSZ,
                rt_bindings::RT_SERIAL_FLOWCONTROL_NONE,
                0, // reserved
            ),
        };
        config
    }
}

pub fn uart_init() {
    let config = rt_bindings::serial_configure::default();

    #[cfg(feature = "enable_uart0")]
    unsafe {
        let serial = UART0_INSTANCE.serial.assume_init_mut();
        serial.ops = &UART_OPS;
        serial.config = config;

        rt_bindings::rt_hw_serial_register(
            serial as *mut _,
            UART0_INSTANCE.name,
            rt_bindings::RT_DEVICE_FLAG_RDWR | rt_bindings::RT_DEVICE_FLAG_INT_RX,
            &raw mut UART0_INSTANCE as *mut _ as *mut core::ffi::c_void,
        );
    }

    #[cfg(feature = "enable_uart1")]
    unsafe {
        let serial = UART1_INSTANCE.serial.assume_init_mut();
        serial.ops = &UART_OPS;
        serial.config = config;

        rt_bindings::rt_hw_serial_register(
            serial as *mut _,
            UART1_INSTANCE.name,
            rt_bindings::RT_DEVICE_FLAG_RDWR | rt_bindings::RT_DEVICE_FLAG_INT_RX,
            &raw mut UART1_INSTANCE as *mut _ as *mut core::ffi::c_void,
        );
    }
}
