#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

#![no_std]

use core::include;
use core::env;

include!(env!("BINDGEN_DIR"));
