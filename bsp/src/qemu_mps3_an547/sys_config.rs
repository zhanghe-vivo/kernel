#![allow(dead_code)]
use crate::kernel::{c_str, str::CStr};

/* ================================================================================ */
/* ================              Peripheral memory map             ================ */
/* ================================================================================ */

/* Secure Subsystem peripheral region */
pub const SYSTIMER0_ARMV8_M_BASE_S: u32 = 0x58000000;
pub const SYSTIMER1_ARMV8_M_BASE_S: u32 = 0x58001000;
pub const SYSTIMER2_ARMV8_M_BASE_S: u32 = 0x58002000;
pub const SYSTIMER3_ARMV8_M_BASE_S: u32 = 0x58003000;
pub const SLOWCLK_WDOG_CMSDK_BASE_S: u32 = 0x5802E000;
pub const SLOWCLK_TIMER_CMSDK_BASE_S: u32 = 0x5802F000;
pub const SYSWDOG_ARMV8_M_CNTRL_BASE_S: u32 = 0x58040000;
pub const SYSWDOG_ARMV8_M_REFRESH_BASE_S: u32 = 0x58041000;
pub const SYSCNTR_CNTRL_BASE_S: u32 = 0x58100000;
pub const SYSCNTR_READ_BASE_S: u32 = 0x58101000;

/* Secure MSTEXPPIHL Peripheral region */
pub const UART0_BASE_S: u32 = 0x59303000;
pub const UART1_BASE_S: u32 = 0x59304000;
pub const UART2_BASE_S: u32 = 0x59305000;
pub const UART3_BASE_S: u32 = 0x59306000;
pub const UART4_BASE_S: u32 = 0x59307000;

pub const SYSTEM_CORE_CLOCK: u32 = 25000000;
pub const TICK_PER_SECOND: u32 = 100;

pub const UART0_CLOCK: u32 = 25000000;
pub const UART0_NAME: &CStr = c_str!("uart0");
pub const UART1_CLOCK: u32 = 25000000;
pub const UART1_NAME: &CStr = c_str!("uart1");
pub const CONSOLE_DEVICE_NAME: *const core::ffi::c_char = UART0_NAME.as_char_ptr();
