// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) vivo

#![no_std]
#![feature(naked_functions)]
#![feature(stmt_expr_attributes)]
#![feature(linkage)]
#![allow(unused)]

/// Maximum number of addresses to store in a backtrace
const MAX_BACKTRACE_ADDRESSES: usize = 10;
#[cfg(all(cortex_m, target_os = "none"))]
pub mod arm_cortex_m;
#[cfg(all(cortex_m, target_os = "none"))]
pub use crate::arm_cortex_m as arch;
// #[link_section] is only usable from the root crate.
// See https://github.com/rust-lang/rust/issues/67209.
#[cfg(all(cortex_m, target_os = "none"))]
include!("arm_cortex_m/handlers.rs");

// #[cfg(all(armv7a, target_os = "none"))]
// pub mod cortex_a;
// #[cfg(all(armv7a, target_os = "none"))]
// pub use crate::cortex_a as arch;

// #[cfg(all(target_arch = "aarch64", target_os = "none"))]
// pub mod aarch64;
// #[cfg(all(target_arch = "aarch64", target_os = "none"))]
// pub use crate::aarch64 as arch;
