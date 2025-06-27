#[cfg(target_arch = "arm")]
pub(crate) mod arm;
#[cfg(target_arch = "arm")]
pub(crate) use arm::*;

#[cfg(target_arch = "riscv64")]
pub(crate) mod riscv64;
#[cfg(target_arch = "riscv64")]
pub(crate) use riscv64::*;

#[cfg(target_arch = "aarch64")]
pub(crate) mod aarch64;
#[cfg(target_arch = "aarch64")]
pub(crate) use aarch64::*;
