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

use alloc::boxed::Box;
#[allow(unused)]
use alloc::{string::String, vec};
use blueos::{
    allocator,
    net::{self, SocketAddress},
    scheduler,
    sync::atomic_wait as futex,
    thread::{Builder as ThreadBuilder, Entry, Stack},
};
use blueos_test_macro::test;
use core::{
    ffi::c_void,
    mem,
    net::SocketAddr,
    sync::atomic::{AtomicUsize, Ordering},
};
use libc::{AF_INET, AF_INET6, IPPROTO_ICMP, SOCK_DGRAM, SOCK_RAW};
use semihosting::println;

use crate::net_utils;

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

static TCP_SERVER_DONE: AtomicUsize = AtomicUsize::new(0);
extern "C" fn tcp_server_thread(_arg: *mut core::ffi::c_void) {
    println!("Thread enter:[tcp_server_thread]");

    // TODO SO_NONBLOCK add to libc
    // Create socket with O_NONBLOCK flag
    // let sock_fd = net::syscalls::socket(AF_INET, libc::SOCK_STREAM | libc::SO_NONBLOCK, 0);

    // Create socket without O_NONBLOCK flag
    let sock_fd = net::syscalls::socket(AF_INET, libc::SOCK_STREAM, 0);

    let ip_addr = "127.0.0.1"; // Replace with actual IP address
    let port = 1234;

    let server_addr = net_utils::create_ipv4_sockaddr(ip_addr, port);

    // Convert sockaddr_in to sockaddr and call bind
    let _ = net::syscalls::bind(
        sock_fd,
        &server_addr as *const _ as *const libc::sockaddr,
        mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
    );

    // Start listening
    let _listen_result = net::syscalls::listen(sock_fd, 0);

    let size = 32 << 10;
    let tcp_client_base = allocator::malloc_align(size, 16);
    start_test_thread(
        "tcp_client_thread",
        tcp_client_thread,
        tcp_client_base as usize,
        size,
    );

    // Create read buffer
    let mut buffer = vec![0u8; 1024];

    let mut bytes_received = 0;
    loop_with_io_mode(move || {
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
            net_utils::println_hex(buffer.as_slice(), received_size);
        }

        scheduler::yield_me();
        true
    });
    net::syscalls::shutdown(sock_fd, 0);

    TCP_SERVER_DONE.store(1, Ordering::Relaxed);
    let _ = futex::atomic_wake(&TCP_SERVER_DONE, 1);
    // assert_eq!(bytes_received > 0, true);
    println!("Thread exit:[tcp_server_thread]");
}

static TCP_CLIENT_DONE: AtomicUsize = AtomicUsize::new(0);
extern "C" fn tcp_client_thread(_arg: *mut core::ffi::c_void) {
    println!("Thread enter:[tcp_client_thread]");

    // Create socket with O_NONBLOCK flag
    // let sock_fd = net::syscalls::socket(AF_INET, libc::SOCK_STREAM | libc::SO_NONBLOCK, 0);

    // Create socket with blocking mode
    let sock_fd = net::syscalls::socket(AF_INET, libc::SOCK_STREAM, 0);

    let ip_addr = "127.0.0.1"; // Replace with actual IP address
    let port = 1234;

    let server_addr = net_utils::create_ipv4_sockaddr(ip_addr, port);

    // Convert sockaddr_in to sockaddr and call connect
    let _ = net::syscalls::connect(
        sock_fd,
        &server_addr as *const _ as *const libc::sockaddr,
        mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
    );

    let message = "Hello From Posix client";
    let bytes = message.as_bytes();

    let mut bytes_sent = 0;
    loop_with_io_mode(move || {
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

    let _ = futex::atomic_wait(&TCP_SERVER_DONE, 0, None);
    // Warning!!! Shutdown after server thread exit, or server may not able to recv data from client
    net::syscalls::shutdown(sock_fd, 0);

    TCP_CLIENT_DONE.store(1, Ordering::Relaxed);
    let _ = futex::atomic_wake(&TCP_CLIENT_DONE, 1);
    // assert_eq!(bytes_sent > 0, true);
    println!("Thread exit:[tcp_client_thread]");
}

static UDP_SERVER_DONE: AtomicUsize = AtomicUsize::new(0);
extern "C" fn udp_server_thread(_arg: *mut core::ffi::c_void) {
    println!("[udp_server_thread] enter");

    // Create socket
    let sock_fd = net::syscalls::socket(AF_INET, SOCK_DGRAM, 0);

    // Bind socket to local host
    let local_ipv4_addr = "127.0.0.1"; // Replace with actual IP address
    let local_port = 1234;
    let local_ipv4_endpoint = net_utils::create_ipv4_sockaddr(local_ipv4_addr, local_port);

    // Convert sockaddr_in to sockaddr and call bind
    let _bind_result = net::syscalls::bind(
        sock_fd,
        &local_ipv4_endpoint as *const _ as *const libc::sockaddr,
        mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
    );

    let size = 32 << 10;
    let udp_client_base = allocator::malloc_align(size, 16);
    start_test_thread(
        "udp_client_thread",
        udp_client_thread,
        udp_client_base as usize,
        size,
    );

    // Create read buffer
    let mut buffer = vec![0u8; 1024];

    let mut addr: libc::sockaddr = unsafe { mem::zeroed() };
    let mut addr_len: libc::socklen_t = unsafe { mem::zeroed() };

    let mut bytes_received = 0;
    loop_with_io_mode(move || {
        let addr_ptr = &mut addr as *mut libc::sockaddr;
        let addr_len_ptr = &mut addr_len as *mut libc::socklen_t;

        // TODO add sockaddr reply
        bytes_received = net::syscalls::recvfrom(
            sock_fd,
            buffer.as_mut_ptr() as *mut c_void,
            buffer.len(),
            0,
            addr_ptr,
            addr_len_ptr,
        );

        if bytes_received > 0 {
            let received_size = bytes_received as usize;
            println!("Received {} bytes of data", bytes_received);

            // Try to convert using String::from_utf8
            match String::from_utf8(buffer[0..received_size].to_vec()) {
                Ok(text) => println!("Received UDP text: {}", text),
                Err(_) => println!("Received UDP data is not valid UTF-8 text"),
            }

            // Hex print section
            net_utils::println_hex(buffer.as_slice(), received_size);

            // print socket addr
            let _ = unsafe {
                SocketAddress::from_ptr(addr_ptr as *const libc::sockaddr, *addr_len_ptr)
            }
            .and_then(|addr| addr.create_ip_endpoint())
            .map(|e| println!("Recv udp packet from ={:#?}", e))
            .or_else(|| {
                println!("Recv udp packet from : find no endpoint");
                None
            });
        }

        scheduler::yield_me();
        true
    });
    net::syscalls::shutdown(sock_fd, 0);

    UDP_SERVER_DONE.store(1, Ordering::Relaxed);
    let _ = futex::atomic_wake(&UDP_SERVER_DONE, 1);
    // assert_eq!(bytes_received > 0, true);
    println!("[udp_server_thread] exit");
}

static UDP_CLIENT_DONE: AtomicUsize = AtomicUsize::new(0);
extern "C" fn udp_client_thread(_arg: *mut core::ffi::c_void) {
    println!("[udp_client_thread] enter");

    // Create socket
    let sock_fd = net::syscalls::socket(AF_INET, SOCK_DGRAM, 0);

    // Prepare remote IP address and port
    let remote_addr = "127.0.0.1"; // Replace with actual IP address
    let remote_port = 1234;
    let remote_endpoint = net_utils::create_ipv4_sockaddr(remote_addr, remote_port);

    // Prepare local IP address and port
    let local_addr = "127.0.0.1"; // Replace with actual IP address
    let local_port = 1235;
    let local_endpoint = net_utils::create_ipv4_sockaddr(local_addr, local_port);

    // Convert sockaddr_in to sockaddr and call bind
    let _bind_result = net::syscalls::bind(
        sock_fd,
        &local_endpoint as *const _ as *const libc::sockaddr,
        mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
    );

    let message = "Hello UDP From Posix client";
    let bytes = message.as_bytes();

    let mut bytes_sent = 0;
    loop_with_io_mode(move || {
        // Call send function to send data
        let bytes_sent = net::syscalls::sendto(
            sock_fd,
            bytes.as_ptr() as *const c_void,
            bytes.len(),
            0,
            &remote_endpoint as *const _ as *const libc::sockaddr,
            mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
        );

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

    let _ = futex::atomic_wait(&UDP_SERVER_DONE, 0, None);
    // Warning!!! Shutdown after server thread exit, or server may not able to recv data from client
    net::syscalls::shutdown(sock_fd, 0);

    UDP_CLIENT_DONE.store(1, Ordering::Relaxed);
    let _ = futex::atomic_wake(&UDP_CLIENT_DONE, 1);
    // assert_eq!(bytes_sent > 0, true);
    println!("[udp_client_thread] exit");
}

static ICMP_THREAD_DONE: AtomicUsize = AtomicUsize::new(0);
extern "C" fn icmp_thread_loop(_arg: *mut core::ffi::c_void) {
    println!("[icmp_thread_loop] enter");

    // Create icmpv4 socket
    let sock_fd = net::syscalls::socket(AF_INET, SOCK_RAW, IPPROTO_ICMP);

    // Create a icmpv4 libc::msghdr
    let mut sockaddr_in_obj = net_utils::create_ipv4_sockaddr("127.0.0.1", 1234);
    // Create ICMPv4 ECHO Msg
    let mut icmp_echo_packet = net_utils::create_icmpv4_echo_packet();

    // // Create icmpv6 socket
    // let sock_fd = net::syscalls::socket(AF_INET6, SOCK_RAW, IPPROTO_ICMPV6);
    // // Create a icmpv6 libc::msghdr
    // let mut sockaddr_in_obj = net_utils::create_ipv6_local_sockaddr(1234);
    // // Create ICMPv6 ECHO Msg
    // let mut icmp_echo_packet = net_utils::create_icmpv6_echo_packet();

    // When bind , we choose udp to recv msg
    // let _ = net::syscalls::bind(
    //     sock_fd,
    //     &sockaddr_in_obj as *const _ as *const libc::sockaddr,
    //     mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
    // );

    let packet_len = icmp_echo_packet.as_slice().len();
    let mut iovec_obj = libc::iovec {
        iov_base: icmp_echo_packet.as_mut_ptr() as *mut libc::c_void,
        iov_len: packet_len,
    };

    // let len_of_sockaddr = core::mem::size_of::<libc::sockaddr_in6>() as libc::socklen_t;
    let len_of_sockaddr = core::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t;
    println!("size_of_sockaddr = {}", len_of_sockaddr);

    let msghdr_send = libc::msghdr {
        // Using IPv4 address
        msg_name: &mut sockaddr_in_obj as *mut libc::sockaddr_in as *mut libc::c_void,
        msg_namelen: core::mem::size_of::<libc::sockaddr_in>() as u32,

        // // Using IPv6 address
        // msg_name: &mut sockaddr_in_obj as *mut libc::sockaddr_in6 as *mut libc::c_void,
        // msg_namelen: len_of_sockaddr,
        msg_iov: &mut iovec_obj as *mut libc::iovec,
        msg_iovlen: 1,
        msg_control: core::ptr::null_mut(),
        msg_controllen: 0,
        msg_flags: 0,
    };

    let msg_send_ptr: *const libc::msghdr = &msghdr_send;

    // Send loop
    loop_with_io_mode(move || {
        let send_bytes = net::syscalls::sendmsg(sock_fd, msg_send_ptr, 0);
        if send_bytes > 0 {
            println!("Send icmp packet bytes={}", send_bytes);
            return false;
        }

        scheduler::yield_me();
        true
    });

    let len_of_iov: usize = 128;
    let mut packet_data = vec![0u8; len_of_iov];
    let mut addr_buffer: libc::sockaddr = unsafe { mem::zeroed() };

    let mut iov = libc::iovec {
        iov_base: packet_data.as_mut_ptr() as *mut libc::c_void,
        iov_len: len_of_iov as libc::size_t,
    };

    let mut msghdr_recv = libc::msghdr {
        msg_name: &mut addr_buffer as *mut _ as *mut libc::c_void,
        msg_namelen: core::mem::size_of::<libc::sockaddr>() as libc::socklen_t,
        msg_iov: &mut iov as *mut libc::iovec,
        msg_iovlen: 1,
        msg_control: core::ptr::null_mut(),
        msg_controllen: 0,
        msg_flags: 0,
    };
    let msg_recv_ptr: *mut libc::msghdr = &mut msghdr_recv;

    semihosting::println!(
        "before recive addr=0x{:x} len={} msg_iovlen={} buffer_len={} msg_iovlen={}",
        msg_recv_ptr as usize,
        core::mem::size_of::<libc::msghdr>(),
        msghdr_recv.msg_iovlen,
        unsafe { { (*msghdr_recv.msg_iov.add(0)) }.iov_len },
        msghdr_recv.msg_iovlen
    );

    let mut recv_bytes = 0;
    // Recv loop
    loop_with_io_mode(move || {
        unsafe { net_utils::print_libc_msghdr(&*msg_recv_ptr) }

        recv_bytes = net::syscalls::recvmsg(sock_fd, msg_recv_ptr, 0);
        if recv_bytes > 0 {
            println!("Recv icmp packet bytes={}", recv_bytes);

            // print data
            let recv_packet = net_utils::collect_iovec_data_with_recv_bytes(
                msghdr_recv.msg_iov,
                msghdr_recv.msg_iovlen as usize,
                recv_bytes as usize,
            )
            .map(|recv_packet| net_utils::println_hex(recv_packet.as_slice(), recv_packet.len()))
            .map_err(|e| println!("Receive data fail: {}", e));

            // print socket addr
            let _ = unsafe {
                SocketAddress::from_ptr(
                    msghdr_recv.msg_name as *const libc::sockaddr,
                    msghdr_recv.msg_namelen,
                )
            }
            .and_then(|addr| addr.create_ip_endpoint())
            .map(|e| println!("Recv icmp packet from ={:#?}", e))
            .or_else(|| {
                println!("Recv icmp packet from : find no endpoint");
                None
            });

            return false; // stop loop
        }

        scheduler::yield_me();
        true
    });

    net::syscalls::shutdown(sock_fd, 0);

    ICMP_THREAD_DONE.store(1, Ordering::Relaxed);
    let _ = futex::atomic_wake(&ICMP_THREAD_DONE, 1);

    println!("[icmp_thread_loop] exit");
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
fn test_posix_api_tcp() {
    println!("Enter test_posix_api_tcp");

    let size = 32 << 10;
    let tcp_server_base = allocator::malloc_align(size, 16);
    start_test_thread(
        "tcp_server_thread",
        tcp_server_thread,
        tcp_server_base as usize,
        size,
    );

    let _ = futex::atomic_wait(&TCP_CLIENT_DONE, 0, None);
}

fn test_posix_api_tcp_non_blocking() {
    println!("Enter test_posix_api_tcp");

    let size = 32 << 10;
    let tcp_server_base = allocator::malloc_align(size, 16);
    start_test_thread(
        "tcp_server_thread",
        tcp_server_thread,
        tcp_server_base as usize,
        size,
    );

    let _ = futex::atomic_wait(&TCP_CLIENT_DONE, 0, None);
    let _ = futex::atomic_wait(&TCP_SERVER_DONE, 0, None);
}

#[test]
fn test_posix_api_udp() {
    println!("Enter test_posix_api_udp");

    let size = 32 << 10;
    let udp_server_base = allocator::malloc_align(size, 16);

    start_test_thread(
        "udp_server_thread",
        udp_server_thread,
        udp_server_base as usize,
        size,
    );

    let _ = futex::atomic_wait(&UDP_CLIENT_DONE, 0, None);
}

#[test]
fn test_posix_api_icmp() {
    println!("Enter test_posix_api_icmp");

    let size = 32 << 10;
    let icmp_base = allocator::malloc_align(size, 16);
    start_test_thread(
        "icmp_thread_loop",
        icmp_thread_loop,
        icmp_base as usize,
        size,
    );
    let _ = futex::atomic_wait(&ICMP_THREAD_DONE, 0, None);
}
