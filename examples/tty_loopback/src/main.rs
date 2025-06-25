#![no_main]
#![no_std]

extern crate alloc;
extern crate rsrt;
use bluekernel::{
    scheduler,
    thread::Thread,
    vfs::syscalls::{vfs_open, vfs_read, vfs_write},
};
use core::ffi::c_char;
use libc::*;
use log::info;

#[no_mangle]
fn main() -> i32 {
    info!("Hello, Blue Kernel!");

    let path = b"/dev/ttyS0\0".as_ptr() as *const c_char;
    let file = vfs_open(path, libc::O_RDWR | libc::O_NONBLOCK, 0);
    loop {
        let mut read_buf = [0u8; 32];
        let read_size = vfs_read(file, read_buf.as_mut_ptr(), read_buf.len());
        if read_size > 0 {
            let slice = &read_buf[..(read_size as usize)];
            let _ = vfs_write(file, slice.as_ptr(), slice.len());
        }
        scheduler::yield_me()
    }
}
