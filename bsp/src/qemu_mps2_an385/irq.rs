#![allow(dead_code)]
#![allow(non_upper_case_globals)]
use crate::arch::IrqNumber;

// use some name as arm cmsdk
pub const UART0RX_IRQn: IrqNumber = IrqNumber::new(0);
pub const UART0TX_IRQn: IrqNumber = IrqNumber::new(1);
pub const UART1RX_IRQn: IrqNumber = IrqNumber::new(2);
pub const UART1TX_IRQn: IrqNumber = IrqNumber::new(3);
pub const UART2RX_IRQn: IrqNumber = IrqNumber::new(4);
pub const UART2TX_IRQn: IrqNumber = IrqNumber::new(5);
pub const GPIO0ALL_IRQn: IrqNumber = IrqNumber::new(6);
pub const GPIO1ALL_IRQn: IrqNumber = IrqNumber::new(7);
pub const TIMER0_IRQn: IrqNumber = IrqNumber::new(8);
pub const TIMER1_IRQn: IrqNumber = IrqNumber::new(9);
pub const DUALTIMER_IRQn: IrqNumber = IrqNumber::new(10);
pub const SPI_0_1_IRQn: IrqNumber = IrqNumber::new(11);
pub const UART_0_1_2_OVF_IRQn: IrqNumber = IrqNumber::new(12);
pub const ETHERNET_IRQn: IrqNumber = IrqNumber::new(13);
pub const I2S_IRQn: IrqNumber = IrqNumber::new(14);
pub const TOUCHSCREEN_IRQn: IrqNumber = IrqNumber::new(15);
pub const GPIO2_IRQn: IrqNumber = IrqNumber::new(16);
pub const GPIO3_IRQn: IrqNumber = IrqNumber::new(17);
pub const UART3RX_IRQn: IrqNumber = IrqNumber::new(18);
pub const UART3TX_IRQn: IrqNumber = IrqNumber::new(19);
pub const UART4RX_IRQn: IrqNumber = IrqNumber::new(20);
pub const UART4TX_IRQn: IrqNumber = IrqNumber::new(21);
pub const SPI_2_IRQn: IrqNumber = IrqNumber::new(22);
pub const SPI_3_4_IRQn: IrqNumber = IrqNumber::new(23);
pub const GPIO0_0_IRQn: IrqNumber = IrqNumber::new(24);
pub const GPIO0_1_IRQn: IrqNumber = IrqNumber::new(25);
pub const GPIO0_2_IRQn: IrqNumber = IrqNumber::new(26);
pub const GPIO0_3_IRQn: IrqNumber = IrqNumber::new(27);
pub const GPIO0_4_IRQn: IrqNumber = IrqNumber::new(28);
pub const GPIO0_5_IRQn: IrqNumber = IrqNumber::new(29);
pub const GPIO0_6_IRQn: IrqNumber = IrqNumber::new(30);
pub const GPIO0_7_IRQn: IrqNumber = IrqNumber::new(31);
