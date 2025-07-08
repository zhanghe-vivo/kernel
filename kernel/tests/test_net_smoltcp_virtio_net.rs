// Copyright (c) 2025 vivo Mobile Communication Co., Ltd.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//       http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Socket test with qemu virtio net device
//!
//! To run the test, we need a tcp server running on other machine , here is a simple server :
//! ## A simple server test script in Node.js
//! ```javascript
//! const net = require('net');
//!
//! const server = net.createServer((socket) => {
//!   console.log('Client connected!');
//!
//!   socket.on('data', (data) => {
//!     console.log(`Recived from client: ${data.toString()}`);
//!     socket.write('Hello from Server!'); // Reply to client
//!   });
//!
//!   socket.on('end', () => {
//!     console.log('Client disconnected!');
//!   });
//!
//!   socket.on('error', (err) => {
//!     console.error(`Client error: ${err.message}`);
//!   });
//! });
//!
//! server.on('error', (err) => {
//!   console.error(`Server error: ${err.message}`);
//! });
//!
//! server.listen(3000, () => {
//!   console.log('Server listening on port 3000');
//! });
//! ```
//!
//! ## Usage Instructions
//! 1. Save as `test_server.js`
//! 2. Install Node.js: https://nodejs.org
//! 3. Run: `node test_server.js`
//! 4. Run test_virtio_net_device() test
//!
#![allow(unused)]
use crate::net_utils;
use alloc::{boxed::Box, string::String, vec};
use blueos::{
    allocator, net, scheduler,
    sync::atomic_wait as futex,
    thread::{Builder as ThreadBuilder, Entry, Stack},
};
use blueos_test_macro::test;
use core::{
    ffi::c_void,
    sync::atomic::{AtomicUsize, Ordering},
};
use semihosting::println;
use smoltcp::{
    iface::{Config, Interface, SocketSet},
    socket::tcp,
    time::{Duration, Instant},
    wire::{EthernetAddress, IpAddress, IpCidr, Ipv4Address},
};

pub type NetThreadFn = extern "C" fn(arg: *mut core::ffi::c_void);

const TEST_BLOCK_MODE: usize = 1;
const TEST_NONBLOCK_MODE: usize = 30;
const TEST_IO_MODE: usize = TEST_BLOCK_MODE;

fn loop_with_io_mode<T: FnMut() -> bool>(mut f: T) {
    loop_with_times(TEST_IO_MODE, f);
}

fn loop_with_times<T: FnMut() -> bool>(times: usize, mut f: T) {
    for _num in 0..times {
        if !f() {
            break;
        }
    }
}

static TCP_CLIENT_DONE: AtomicUsize = AtomicUsize::new(0);
extern "C" fn tcp_client_virtio_net(_arg: *mut core::ffi::c_void) {
    println!("Thread enter:[tcp_client_virtio_net]");

    // Create socket with O_NONBLOCK flag
    let sock_fd = net::syscalls::socket(libc::AF_INET, libc::SOCK_STREAM | libc::SO_NONBLOCK, 0);

    // Create socket with blocking mode
    // let sock_fd = net::syscalls::socket(libc::AF_INET, libc::SOCK_STREAM, 0);

    // Connect to host server at 10.58.54.123:3000
    // In QEMU user networking, the host is accessible via 10.0.2.2
    // let remote_addr = (Ipv4Address::new(10, 0, 2, 2), 5001);
    // Window Subsystem Linux
    let remote_addr = "10.58.54.123"; // Replace with actual IP address
    let remote_port = 3000;

    let server_addr = net_utils::create_ipv4_sockaddr(remote_addr, remote_port);

    // Convert sockaddr_in to sockaddr and call connect
    let _ = net::syscalls::connect(
        sock_fd,
        &server_addr as *const _ as *const libc::sockaddr,
        core::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
    );

    let message = "Hello From Posix client";
    let bytes = message.as_bytes();

    let mut bytes_sent = 0;
    loop_with_times(10, move || {
        // Call send function to send data
        bytes_sent = net::syscalls::send(sock_fd, bytes.as_ptr() as *const c_void, bytes.len(), 0);

        // Handle send result
        if bytes_sent >= 0 {
            println!("Successfully sent {} bytes of data", bytes_sent);
            println!("Sent message: {}", message);

            if bytes_sent as usize != bytes.len() {
                println!(
                    "Warning: Only sent partial data ({}/{} bytes)",
                    bytes_sent,
                    bytes.len()
                );
            }
        } else {
            println!("Failed to send data");
        }

        scheduler::yield_me();
        true
    });

    // Create read buffer
    let mut buffer = vec![0u8; 1024];

    if bytes_sent > 0 {
        let mut bytes_received = 0;
        loop_with_times(5, move || {
            bytes_received =
                net::syscalls::recv(sock_fd, buffer.as_mut_ptr() as *mut c_void, buffer.len(), 0);

            if bytes_received > 0 {
                let received_size = bytes_received as usize;
                println!("Received {} bytes of data", bytes_received);

                // Try to convert using String::from_utf8
                match String::from_utf8(buffer[0..received_size].to_vec()) {
                    Ok(text) => println!("Received text: {}", text),
                    Err(_) => println!("Received data is not valid UTF-8 text"),
                }

                // Hex print section
                net_utils::println_hex(&buffer, received_size);
            }

            scheduler::yield_me();
            true
        });
    }

    let _ = net::syscalls::shutdown(sock_fd, 0);

    println!("Thread exit:[tcp_client_virtio_net]");
    // assert_eq!(bytes_sent > 0 && bytes_received > 0, true);
    TCP_CLIENT_DONE.store(1, Ordering::Relaxed);
    let _ = futex::atomic_wake(&TCP_CLIENT_DONE, 1);
}

pub fn start_test_thread(_thread_name: &str, thread_fn: NetThreadFn, base: usize, size: usize) {
    println!("start_test_thread [{}] at base 0x{:x}", _thread_name, base);
    let t = ThreadBuilder::new(Entry::Posix(thread_fn, core::ptr::null_mut()))
        .set_stack(Stack::Raw { base, size })
        .build();
    t.lock()
        .set_cleanup(Entry::Closure(Box::new(move || unsafe {
            println!("clean up begin 0x{:x}", base);
            allocator::free_align(base as *mut u8, 16);
            println!("clean up finish 0x{:x}", base);
        })));
    scheduler::queue_ready_thread(t.state(), t);
}

#[test]
fn test_virtio_net() {
    println!("Enter test_virtio_net");

    let size = 32 << 10;
    let base = allocator::malloc_align(size, 16);
    start_test_thread(
        "tcp_client_virtio_net",
        tcp_client_virtio_net,
        base as usize,
        size,
    );
    let _ = futex::atomic_wait(&TCP_CLIENT_DONE, 0, None);
}
