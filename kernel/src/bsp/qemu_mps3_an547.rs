mod cmsdk_uart;
pub mod init;
mod irq;
mod sys_config;
mod systick;
#[cfg(not(feature = "os_adapter"))]
pub mod uart;
#[cfg(feature = "os_adapter")]
mod uart_rt;
#[cfg(feature = "os_adapter")]
use uart_rt as uart;

#[cfg(feature = "enable_uart0")]
pub use uart::UARTRX0_Handler;
#[cfg(feature = "enable_uart1")]
pub use uart::UARTRX1_Handler;
