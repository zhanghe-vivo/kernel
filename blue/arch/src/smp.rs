// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2018-2023 Andre Richter <andre.o.richter@gmail.com>

//! Symmetric multiprocessing.

// #[cfg(target_arch = "aarch64")]
// #[path = "aarch64/smp.rs"]
// mod arch_smp;

// #[cfg(armv7a)]
// #[path = "cortex_a/smp.rs"]
// mod arch_smp;

#[cfg(all(cortex_m, target_os = "none"))]
#[path = "cortex_m/smp.rs"]
mod arch_smp;

//--------------------------------------------------------------------------------------------------
// Architectural Public Reexports
//--------------------------------------------------------------------------------------------------
pub use arch_smp::core_id;
