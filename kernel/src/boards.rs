#[cfg(target_board = "qemu_mps2_an385")]
mod qemu_mps2_an385;
#[cfg(target_board = "qemu_mps2_an385")]
pub use qemu_mps2_an385::*;

#[cfg(target_board = "qemu_mps3_an547")]
mod qemu_mps3_an547;
#[cfg(target_board = "qemu_mps3_an547")]
pub use qemu_mps3_an547::*;

#[cfg(target_board = "qemu_virt64_aarch64")]
mod qemu_virt64_aarch64;
#[cfg(target_board = "qemu_virt64_aarch64")]
pub use qemu_virt64_aarch64::*;
