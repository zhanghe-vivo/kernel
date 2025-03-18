#![allow(dead_code)]
#![allow(non_upper_case_globals)]
use crate::arch::IrqNumber;

// use some name as arm cmsdk
pub const NONSEC_WATCHDOG_RESET_REQ_IRQn: IrqNumber = IrqNumber::new(0); // Non-Secure Watchdog Reset Request Interrupt
pub const NONSEC_WATCHDOG_IRQn: IrqNumber = IrqNumber::new(1); // Non-Secure Watchdog Interrupt
pub const SLOWCLK_TIMER_IRQn: IrqNumber = IrqNumber::new(2); // SLOWCLK Timer Interrupt
pub const TIMER0_IRQn: IrqNumber = IrqNumber::new(3); // TIMER 0 Interrupt
pub const TIMER1_IRQn: IrqNumber = IrqNumber::new(4); // TIMER 1 Interrupt
pub const TIMER2_IRQn: IrqNumber = IrqNumber::new(5); // TIMER 2 Interrupt
pub const MPC_IRQn: IrqNumber = IrqNumber::new(9); // MPC Combined (Secure) Interrupt
pub const PPC_IRQn: IrqNumber = IrqNumber::new(10); // PPC Combined (Secure) Interrupt
pub const MSC_IRQn: IrqNumber = IrqNumber::new(11); // MSC Combined (Secure) Interrput
pub const BRIDGE_ERROR_IRQn: IrqNumber = IrqNumber::new(12); // Bridge Error Combined (Secure) Interrupt
pub const MGMT_PPU_IRQn: IrqNumber = IrqNumber::new(14); // MGMT PPU
pub const SYS_PPU_IRQn: IrqNumber = IrqNumber::new(15); // SYS PPU
pub const CPU0_PPU_IRQn: IrqNumber = IrqNumber::new(16); // CPU0 PPU
pub const DEBUG_PPU_IRQn: IrqNumber = IrqNumber::new(26); // DEBUG PPU
pub const TIMER3_AON_IRQn: IrqNumber = IrqNumber::new(27); // TIMER 3 AON Interrupt
pub const CPU0_CTI_0_IRQn: IrqNumber = IrqNumber::new(28); // CPU0 CTI IRQ 0
pub const CPU0_CTI_1_IRQn: IrqNumber = IrqNumber::new(29); // CPU0 CTI IRQ 1
pub const System_Timestamp_Counter_IRQn: IrqNumber = IrqNumber::new(32); // System timestamp counter Interrupt
pub const UARTTX0_IRQn: IrqNumber = IrqNumber::new(33); // UART 0 TX Interrupt
pub const UARTRX0_IRQn: IrqNumber = IrqNumber::new(34); // UART 0 RX Interrupt
pub const UARTRX1_IRQn: IrqNumber = IrqNumber::new(35); // UART 1 RX Interrupt
pub const UARTTX1_IRQn: IrqNumber = IrqNumber::new(36); // UART 1 TX Interrupt
pub const UARTRX2_IRQn: IrqNumber = IrqNumber::new(37); // UART 2 RX Interrupt
pub const UARTTX2_IRQn: IrqNumber = IrqNumber::new(38); // UART 2 TX Interrupt
pub const UARTRX3_IRQn: IrqNumber = IrqNumber::new(39); // UART 3 RX Interrupt
pub const UARTTX3_IRQn: IrqNumber = IrqNumber::new(40); // UART 3 TX Interrupt
pub const UARTRX4_IRQn: IrqNumber = IrqNumber::new(41); // UART 4 RX Interrupt
pub const UARTTX4_IRQn: IrqNumber = IrqNumber::new(42); // UART 4 TX Interrupt
