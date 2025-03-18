use super::{
    cmsdk_uart::Uart,
    sys_config::{SYSTEM_CORE_CLOCK, UART0_BASE_S},
};

pub static mut UART0: Uart = unsafe { Uart::new(UART0_BASE_S as *mut u32) };

pub fn uart_init() {
    unsafe { UART0.init(SYSTEM_CORE_CLOCK, 115200) };
}
