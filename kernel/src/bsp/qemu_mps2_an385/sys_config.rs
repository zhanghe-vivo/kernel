#![allow(dead_code)]
use crate::kernel::c_str;
use core::ffi::CStr;

/* ================================================================================ */
/* ================              Peripheral memory map             ================ */
/* ================================================================================ */

/* Peripheral and SRAM base address */
pub const FLASH_BASE: u32 = 0x00000000;
pub const SRAM_BASE: u32 = 0x20000000;
pub const PERIPH_BASE: u32 = 0x40000000;

pub const RAM_BASE: u32 = 0x20000000;
pub const APB_BASE: u32 = 0x40000000;
pub const AHB_BASE: u32 = 0x40010000;

/* APB peripherals */
pub const TIMER0_BASE: u32 = APB_BASE + 0x0000;
pub const TIMER1_BASE: u32 = APB_BASE + 0x1000;
pub const DUALTIMER_BASE: u32 = APB_BASE + 0x2000;
pub const DUALTIMER_1_BASE: u32 = DUALTIMER_BASE;
pub const DUALTIMER_2_BASE: u32 = DUALTIMER_BASE + 0x20;
pub const UART0_BASE: u32 = APB_BASE + 0x4000;
pub const UART1_BASE: u32 = APB_BASE + 0x5000;
pub const UART2_BASE: u32 = APB_BASE + 0x6000;
pub const WATCHDOG_BASE: u32 = APB_BASE + 0x8000;

/* AHB peripherals */
pub const GPIO0_BASE: u32 = AHB_BASE + 0x0000;
pub const GPIO1_BASE: u32 = AHB_BASE + 0x1000;
pub const SYSCTRL_BASE: u32 = AHB_BASE + 0xF000;

pub const SYSTEM_CORE_CLOCK: u32 = 25000000;
pub const TICK_PER_SECOND: u32 = 100;

pub const UART0_NAME: &CStr = c_str!("uart0");
pub const UART1_NAME: &CStr = c_str!("uart1");
pub const CONSOLE_DEVICE_NAME: *const core::ffi::c_char = UART0_NAME.as_ptr();
