#[cfg(cortex_m)]
mod cortex_m;
#[cfg(cortex_m)]
pub use cortex_m::{syscall0, syscall1, syscall2, syscall3, syscall4, syscall5, syscall6};
#[cfg(cortex_a)]
mod cortex_a;
#[cfg(cortex_a)]
pub use cortex_a::{syscall0, syscall1, syscall2, syscall3, syscall4, syscall5, syscall6};
