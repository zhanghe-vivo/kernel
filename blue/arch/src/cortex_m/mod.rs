//! ARM Cortex-M hardware support.

pub mod core;
mod exception;
pub mod interrupt;
mod scheduler;
mod stack_frame;

pub struct Arch;

// re-exports
pub use crate::cortex_m::core::ArchCore;
