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

#![allow(unused)]
use alloc::{format, string::String, vec::Vec};
use blueos::net::{SocketAddress, SocketDomain};
use core::{
    ffi::{c_int, c_void},
    fmt::Write,
    mem,
    mem::MaybeUninit,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    slice,
    time::Duration,
};
use libc::{sockaddr_in, sockaddr_in6, AF_INET, AF_INET6, IN6ADDR_LOOPBACK_INIT};
use semihosting::println;
use smoltcp::wire::{IpAddress, IpEndpoint};

/// Get packet data from iovec which using a scatter/gather IO vec
fn collect_iovec_data(iov_ptr: *const libc::iovec, iov_len: usize) -> Result<Vec<u8>, String> {
    if iov_ptr.is_null() {
        return Err("Missing iov pointer".into());
    }

    let iov_array = unsafe { slice::from_raw_parts(iov_ptr, iov_len) };
    let total_size = iov_array.iter().map(|iov| iov.iov_len).sum();
    let mut buffer = Vec::with_capacity(total_size);

    for iov in iov_array {
        if iov.iov_base.is_null() {
            println!("Null pointer in I/O vector");
            continue;
        }

        let data_slice = unsafe { slice::from_raw_parts(iov.iov_base as *const u8, iov.iov_len) };
        buffer.extend_from_slice(data_slice);
    }

    if buffer.is_empty() {
        Err("Null in iovec".into())
    } else {
        Ok(buffer)
    }
}

/// Get packet data from iovec which using a scatter/gather IO vec
/// When recv msg , we only know recv_bytes
pub fn collect_iovec_data_with_recv_bytes(
    iov_ptr: *const libc::iovec,
    iov_len: usize,
    recv_bytes: usize,
) -> Result<Vec<u8>, String> {
    if iov_ptr.is_null() || iov_len == 0 || recv_bytes == 0 {
        return Err("Null iovec pointer".into());
    }

    let iov_array = unsafe { core::slice::from_raw_parts(iov_ptr, iov_len) };
    let mut buffer: Vec<u8> = Vec::with_capacity(recv_bytes);
    let mut remaining = recv_bytes;

    for iov in iov_array {
        if remaining == 0 {
            break;
        }

        if iov.iov_base.is_null() {
            continue;
        }

        let to_read = core::cmp::min(remaining, iov.iov_len as usize);

        unsafe {
            let src = iov.iov_base as *const u8;

            let current_len = buffer.len();
            buffer.resize(current_len + to_read, 0);

            src.copy_to_nonoverlapping(buffer.as_mut_ptr().add(current_len), to_read);
        }

        remaining -= to_read;
    }

    if buffer.is_empty() {
        Err("No valid data read".into())
    } else {
        Ok(buffer)
    }
}

/// Parse libc::msghdr
///
/// # Return
/// - `Some(IpAddress)`: remote address
/// - `Vec<u8>`: data payload , like an icmp packet
///
pub fn parse_msghdr(
    socket_domain: libc::c_int,
    msghdr_ptr: *const libc::msghdr,
) -> Result<(Option<IpEndpoint>, Vec<u8>), String> {
    if msghdr_ptr.is_null() {
        return Err("Null pointer : msghdr".into());
    }

    let msg = unsafe { &*msghdr_ptr };

    let socket_domain = match SocketDomain::try_from(socket_domain) {
        Ok(socket_domain) => socket_domain,
        Err(_) => return Err("Invalid socket domain".into()),
    };

    // Parse remote address
    let remote_endpoint =
        unsafe { SocketAddress::from_ptr(msg.msg_name as *const libc::sockaddr, msg.msg_namelen) }
            .and_then(|addr| addr.create_ip_endpoint());

    // Parse data payload
    let data = collect_iovec_data(msg.msg_iov, msg.msg_iovlen as usize)?;

    Ok((remote_endpoint, data))
}

// Converts an IP address string to network byte order u32
pub fn parse_ipv4_to_network_order(ip_addr: &str) -> u32 {
    let ip_parts: Vec<u8> = ip_addr
        .split('.')
        .map(|octet| {
            octet
                .parse::<u8>()
                .expect("IP address octet should be a valid u8")
        })
        .collect();

    // assert_eq!(ip_parts.len(), 4, "IP address must have exactly 4 octets");

    let ip_bytes = [ip_parts[3], ip_parts[2], ip_parts[1], ip_parts[0]];
    u32::from_be_bytes(ip_bytes)
}

// Creates a sockaddr_in structure for socket operations
pub fn create_ipv4_sockaddr(ip_addr: &str, port: u16) -> libc::sockaddr_in {
    let ip_network_order = parse_ipv4_to_network_order(ip_addr);

    let mut addr: libc::sockaddr_in = unsafe { mem::zeroed() };
    addr.sin_family = libc::AF_INET as libc::sa_family_t;
    addr.sin_port = port; // Do not need network byte order, smoltcp will handle it
    addr.sin_addr.s_addr = ip_network_order;

    addr
}

// Creates a sockaddr_in6 structure for IPv6 socket operations
pub fn create_ipv6_local_sockaddr(port: u16) -> libc::sockaddr_in6 {
    let mut addr: libc::sockaddr_in6 = unsafe { mem::zeroed() };
    addr.sin6_family = libc::AF_INET6 as libc::sa_family_t;
    addr.sin6_port = port; // Do not need network byte order, smoltcp will handle it
    addr.sin6_addr = IN6ADDR_LOOPBACK_INIT;

    addr
}

pub fn create_icmpv4_echo_packet() -> Vec<u8> {
    static ECHO_PACKET_BYTES: [u8; 12] = [
        0x08, // type     : u8    ICMPv4 ECHO = 8
        0x00, // code     : u8
        0x8e, 0xfe, // checksum : u16
        0x02, 0x2b, // identifer: u16   BigEndian , ident   = 0x22b
        0x00, 0x00, // sequence : u16   BinEndian , seq_no  = 0
        0xaa, 0x00, 0x00, 0xff, // data : Vec<u8>
    ];

    Vec::from(ECHO_PACKET_BYTES)
}

pub fn create_icmpv6_echo_packet() -> Vec<u8> {
    static ECHO_PACKET_BYTES: [u8; 12] = [
        0x80, // type     : u8    ICMPv6 ECHO = 0x80
        0x00, // code     : u8
        0x19, 0xb3, // checksum : u16
        0x12, 0x34, // identifer: u16   BigEndian , ident   = 0x1234
        0xab, 0xcd, // sequence : u16   BinEndian , seq_no  = 0
        0xaa, 0x00, 0x00, 0xff, // data : Vec<u8>
    ];

    Vec::from(ECHO_PACKET_BYTES)
}

pub fn create_icmp_msghdr(ip_addr: &str, port: u16) -> libc::msghdr {
    let mut sockaddr_in_obj = create_ipv4_sockaddr(ip_addr, port);

    // Create a ICMP packet , and put into iovec
    let mut icmp_echo_packet = create_icmpv4_echo_packet();
    let packet_len = icmp_echo_packet.as_slice().len();
    let mut iovec_obj = libc::iovec {
        iov_base: icmp_echo_packet.as_mut_ptr() as *mut libc::c_void,
        iov_len: packet_len,
    };

    libc::msghdr {
        msg_name: &mut sockaddr_in_obj as *mut libc::sockaddr_in as *mut libc::c_void,
        msg_namelen: core::mem::size_of::<libc::sockaddr_in>() as u32,
        msg_iov: &mut iovec_obj as *mut libc::iovec,
        msg_iovlen: 1,
        msg_control: core::ptr::null_mut(),
        msg_controllen: 0,
        msg_flags: 0,
    }
}

// print hex data
pub fn println_hex(buffer: &[u8], received_size: usize) {
    println!("Print Hex (hex), size={}: Start----------", received_size);

    for (i, chunk) in buffer[0..received_size].chunks(16).enumerate() {
        // Pre-allocate string buffer (74 = 6 addr + 3*16 bytes + 2 spaces + 8 ascii)
        let mut line = String::with_capacity(74);

        // Write address header
        let _ = write!(&mut line, "[{:04x}]  ", i * 16);

        // Hex dump section
        for (idx, byte) in chunk.iter().enumerate() {
            let _ = write!(&mut line, "{:02x}", byte);
            // Add space between bytes, but not after last byte
            if idx < 15 {
                line.push(' ');
            }
        }

        // Alignment padding for partial lines
        if chunk.len() < 16 {
            let missing = 16 - chunk.len();
            line.extend(core::iter::repeat(' ').take(missing * 3));
        }

        // ASCII visualization section
        line.push_str("  |");
        for byte in chunk {
            let c = match *byte {
                0x20..=0x7e => *byte as char,
                _ => '.',
            };
            line.push(c);
        }
        line.push('|');

        // Single output operation
        println!("{}", line);
    }
    println!("Print Hex (hex), size={}: End----------", received_size);
}

pub fn print_libc_msghdr(hdr: &libc::msghdr) {
    println!(
        "libc::msghdr {{ 
            msg_name: {:p}, 
            msg_namelen: {}, 
            msg_iov: {:p}, 
            msg_iovlen: {}, 
            msg_control: {:p}, 
            msg_controllen: {}, 
            msg_flags: 0x{:x} 
        }}",
        hdr.msg_name,
        hdr.msg_namelen,
        hdr.msg_iov,
        hdr.msg_iovlen,
        hdr.msg_control,
        hdr.msg_controllen,
        hdr.msg_flags
    );
}
