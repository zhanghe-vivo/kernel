use bluekernel::{
    bluekernel_kconfig::MAIN_THREAD_PRIORITY,
    println,
    thread::{Thread, ThreadBuilder},
};
use bluekernel_test_macro::test;
use core::{str, sync::atomic::AtomicUsize};

use smoltcp::{
    iface::{Config, Interface, SocketSet},
    phy::{Device, Loopback, Medium},
    socket::tcp,
    time::{Duration, Instant},
    wire::{EthernetAddress, IpAddress, IpCidr},
};

mod mock {
    use core::cell::Cell;
    use smoltcp::time::{Duration, Instant};

    #[derive(Debug)]
    pub struct Clock(Cell<Instant>);

    impl Clock {
        pub fn new() -> Clock {
            Clock(Cell::new(Instant::from_millis(0)))
        }

        pub fn advance(&self, duration: Duration) {
            self.0.set(self.0.get() + duration)
        }

        pub fn elapsed(&self) -> Instant {
            self.0.get()
        }
    }
}

// Run smoltcp socket test on loopback device.
// WARNING: Do not run this test in the main thread to prevent stack overflow.
extern "C" fn thread_entry(arg: *mut core::ffi::c_void) {
    let clock = mock::Clock::new();
    let mut device = Loopback::new(Medium::Ethernet);

    println!("[smoltcp Tcp Socket Test]: Create interface with loopback device");
    // Create interface
    let mut config = match device.capabilities().medium {
        Medium::Ethernet => {
            Config::new(EthernetAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x01]).into())
        }
        Medium::Ip => Config::new(smoltcp::wire::HardwareAddress::Ip),
        Medium::Ieee802154 => todo!(),
    };

    let mut iface = Interface::new(config, &mut device, Instant::from_millis(0));
    iface.update_ip_addrs(|ip_addrs| {
        ip_addrs
            .push(IpCidr::new(IpAddress::v4(127, 0, 0, 1), 8))
            .unwrap();
    });

    println!("[smoltcp Tcp Socket Test]: Create sockets");
    // Create sockets
    let server_socket = {
        // It is not strictly necessary to use a `static mut` and unsafe code here, but
        // on embedded systems that smoltcp targets it is far better to allocate the data
        // statically to verify that it fits into RAM rather than get undefined behavior
        // when stack overflows.
        static mut TCP_SERVER_RX_DATA: [u8; 1024] = [0; 1024];
        static mut TCP_SERVER_TX_DATA: [u8; 1024] = [0; 1024];
        let tcp_rx_buffer = tcp::SocketBuffer::new(unsafe { &mut TCP_SERVER_RX_DATA[..] });
        let tcp_tx_buffer = tcp::SocketBuffer::new(unsafe { &mut TCP_SERVER_TX_DATA[..] });
        tcp::Socket::new(tcp_rx_buffer, tcp_tx_buffer)
    };

    let client_socket = {
        static mut TCP_CLIENT_RX_DATA: [u8; 1024] = [0; 1024];
        static mut TCP_CLIENT_TX_DATA: [u8; 1024] = [0; 1024];
        let tcp_rx_buffer = tcp::SocketBuffer::new(unsafe { &mut TCP_CLIENT_RX_DATA[..] });
        let tcp_tx_buffer = tcp::SocketBuffer::new(unsafe { &mut TCP_CLIENT_TX_DATA[..] });
        tcp::Socket::new(tcp_rx_buffer, tcp_tx_buffer)
    };

    let mut sockets: [_; 2] = Default::default();
    let mut sockets = SocketSet::new(&mut sockets[..]);
    let server_handle = sockets.add(server_socket);
    let client_handle = sockets.add(client_socket);

    let mut did_listen = false;
    let mut did_connect = false;
    let mut done = false;

    println!("[smoltcp Tcp Socket Test]: Enter poll device loop");
    while !done && clock.elapsed() < Instant::from_millis(10_000) {
        iface.poll(clock.elapsed(), &mut device, &mut sockets);

        let mut socket = sockets.get_mut::<tcp::Socket>(server_handle);
        if !socket.is_active() && !socket.is_listening() {
            if !did_listen {
                println!("[smoltcp Tcp Socket Test]: Socket listening");
                socket.listen(1234).unwrap();
                did_listen = true;
            }
        }

        if socket.can_recv() {
            println!(
                "[smoltcp Tcp Socket Test]: Socket recv {:?}",
                socket.recv(|buffer| { (buffer.len(), str::from_utf8(buffer).unwrap()) })
            );
            socket.close();
            done = true;
        }

        let mut socket = sockets.get_mut::<tcp::Socket>(client_handle);
        let cx = iface.context();
        if !socket.is_open() {
            if !did_connect {
                println!("[smoltcp Tcp Socket Test]: Socket connecting");
                socket
                    .connect(cx, (IpAddress::v4(127, 0, 0, 1), 1234), 65000)
                    .unwrap();
                did_connect = true;
            }
        }

        if socket.can_send() {
            println!("[smoltcp Tcp Socket Test]: Socket sending 0123456789abcdef");
            socket.send_slice(b"0123456789abcdef").unwrap();
            socket.close();
        }

        match iface.poll_delay(clock.elapsed(), &sockets) {
            Some(Duration::ZERO) => println!("[smoltcp Tcp Socket Test]: iface resuming"),
            Some(delay) => {
                println!(
                    "[smoltcp Tcp Socket Test]: Inteface poll sleeping for {} ms",
                    delay
                );
                clock.advance(delay);
                println!("[smoltcp Tcp Socket Test]: after advance")
            }
            None => clock.advance(Duration::from_millis(1)),
        }
    }

    assert!(
        done,
        "[smoltcp Tcp Socket Test]: Bailing out: socket test took too long on loopback device"
    );
}

#[test]
fn test_smoltcp() {
    println!("[smoltcp Integration Test] Enter test_loopback_in_thread");

    // Create new thread using ThreadBuilder
    let thread = ThreadBuilder::default()
        .name(unsafe { core::ffi::CStr::from_bytes_with_nul_unchecked(b"smoltcp\0") })
        .entry_fn(thread_entry)
        .stack_size(32768) // 32KB stack
        .priority(MAIN_THREAD_PRIORITY.try_into().unwrap()) // share cpu time with main thread
        .tick(50)
        .build_from_heap()
        .expect("Failed to create smoltcp test thread");

    unsafe { (&mut *thread.as_ptr()).start() };

    // Sleep a bit to ensure thread starts
    let _ = Thread::msleep(1000);
}
