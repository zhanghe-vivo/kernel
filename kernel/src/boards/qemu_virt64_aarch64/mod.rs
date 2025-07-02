pub mod init;
pub use init::*;
pub mod uart;
pub use uart::get_early_uart;
mod config;

use crate::arch::registers::cntfrq_el0::CNTFRQ_EL0;
use tock_registers::interfaces::Readable;
pub(crate) fn get_cycles_to_duration(cycles: u64) -> core::time::Duration {
    return core::time::Duration::from_nanos(
        (cycles as f64 * (1_000_000_000 as f64 / CNTFRQ_EL0.get() as f64)) as u64,
    );
}

pub(crate) fn get_cycles_to_ms(cycles: u64) -> u64 {
    return (cycles as f64 * (1_000_000 as f64 / CNTFRQ_EL0.get() as f64)) as u64;
}
