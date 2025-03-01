#![no_std]
#![feature(linkage)]

mod rt_bindings;
use bluekernel as kernel;
use bluekernel_arch::arch;

#[cfg(target_board = "qemu_mps2_an385")]
mod qemu_mps2_an385;
// #[link_section] is only usable from the root crate.
// See https://github.com/rust-lang/rust/issues/67209.
#[cfg(target_board = "qemu_mps2_an385")]
include!("qemu_mps2_an385/handlers.rs");

#[cfg(target_board = "qemu_mps3_an547")]
mod qemu_mps3_an547;
#[cfg(target_board = "qemu_mps3_an547")]
include!("qemu_mps3_an547/handlers.rs");
