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
#[cfg(cortex_m)]
pub mod arm_cortex_m;
#[cfg(cortex_m)]
pub use crate::arm_cortex_m as arch;
// #[link_section] is only usable from the root crate.
// See https://github.com/rust-lang/rust/issues/67209.
#[cfg(cortex_m)]
include!("arm_cortex_m/handlers.rs");

#[cfg(target_arch = "aarch64")]
pub mod aarch64_cortex_a;
#[cfg(target_arch = "aarch64")]
pub use crate::aarch64_cortex_a as arch;
#[cfg(target_arch = "aarch64")]
include!("aarch64_cortex_a/exception.rs");
