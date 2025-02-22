#![no_std]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![feature(linkage)]
#![feature(c_size_t)]

pub use blue_arch;
pub use blue_kernel;
// use blue_kernel as kernel;

#[cfg(rt_thread)]
pub mod rt_thread;
