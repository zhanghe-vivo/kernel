#![cfg_attr(not(test), no_std)]
#![cfg_attr(test, feature(test))]
#![allow(internal_features)]
#![feature(box_as_ptr)]
#![feature(box_into_inner)]
#![feature(box_vec_non_null)]
#![feature(c_size_t)]
#![feature(const_trait_impl)]
#![feature(core_intrinsics)]
#![feature(linkage)]
#![feature(negative_impls)]
#![feature(non_null_from_ref)]
#![feature(pointer_is_aligned_to)]
#![feature(ptr_as_uninit)]
#![feature(slice_as_chunks)]

pub mod intrusive;
pub mod list;
pub mod ringbuffer;
pub mod spinarc;
pub mod string;
pub mod tinyarc;
pub mod tinyrwlock;
