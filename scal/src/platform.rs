#[cfg(cortex_m)]
mod cortex_m;
#[cfg(cortex_m)]
pub use cortex_m::{syscall0, syscall1, syscall2, syscall3, syscall4, syscall5, syscall6};
#[cfg(target_arch = "aarch64")]
mod aarch64;
#[cfg(target_arch = "aarch64")]
pub use aarch64::{syscall0, syscall1, syscall2, syscall3, syscall4, syscall5, syscall6};
#[cfg(target_arch = "riscv64")]
mod riscv64;
#[cfg(target_arch = "riscv64")]
pub use riscv64::{syscall0, syscall1, syscall2, syscall3, syscall4, syscall5, syscall6};
