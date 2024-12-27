// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) vivo

#![no_std]
#![feature(naked_functions)]
#![feature(stmt_expr_attributes)]
#![allow(unused)]

#[cfg(all(cortex_m, target_os = "none"))]
pub mod cortex_m;
#[cfg(all(cortex_m, target_os = "none"))]
pub use crate::cortex_m as arch;

// #[cfg(all(armv7a, target_os = "none"))]
// pub mod cortex_a;
// #[cfg(all(armv7a, target_os = "none"))]
// pub use crate::cortex_a as arch;

// #[cfg(all(target_arch = "aarch64", target_os = "none"))]
// pub mod aarch64;
// #[cfg(all(target_arch = "aarch64", target_os = "none"))]
// pub use crate::aarch64 as arch;
