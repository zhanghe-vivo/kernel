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
#[cfg(all(cortex_m))]
pub mod arm_cortex_m;
#[cfg(all(cortex_m))]
pub use crate::arm_cortex_m as arch;
// #[link_section] is only usable from the root crate.
// See https://github.com/rust-lang/rust/issues/67209.
#[cfg(all(cortex_m))]
include!("arm_cortex_m/handlers.rs");

#[cfg(cortex_a)]
use core::arch::global_asm;
#[cfg(cortex_a)]
pub mod aarch64_cortex_a;
#[cfg(cortex_a)]
pub use crate::aarch64_cortex_a as arch;
#[cfg(cortex_a)]
include!("aarch64_cortex_a/exception.rs");
// #[cfg(cortex_a)]
// include!("aarch64_cortex_a/start.rs");
