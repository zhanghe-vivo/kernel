#![cfg_attr(not(test), no_std)]
#![cfg_attr(test, feature(box_as_ptr))]
#![cfg_attr(test, feature(test))]
#![allow(internal_features)]
#![feature(linkage)]
#![feature(c_size_t)]
#![feature(pointer_is_aligned_to)]
#![feature(ptr_as_uninit)]
#![feature(slice_as_chunks)]
#![feature(core_intrinsics)]
#![allow(internal_features)]

pub mod klibc;
pub mod list;
pub mod ringbuffer;
