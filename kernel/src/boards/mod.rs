#[cfg(target_board = "qemu_mps2_an385")]
mod qemu_mps2_an385;
#[cfg(target_board = "qemu_mps2_an385")]
pub(crate) use qemu_mps2_an385::{config, init};

#[cfg(target_board = "qemu_riscv64")]
mod qemu_riscv64;
#[cfg(target_board = "qemu_riscv64")]
pub(crate) use qemu_riscv64::{
    current_cycles, current_ticks, get_early_uart, handle_plic_irq, init, set_timeout_after,
};

#[cfg(target_board = "qemu_mps3_an547")]
mod qemu_mps3_an547;
#[cfg(target_board = "qemu_mps3_an547")]
pub(crate) use qemu_mps3_an547::{config, init};

#[cfg(target_board = "qemu_virt64_aarch64")]
mod qemu_virt64_aarch64;
#[cfg(target_board = "qemu_virt64_aarch64")]
pub(crate) use qemu_virt64_aarch64::{config, init};
