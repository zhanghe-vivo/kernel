#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
use bluekernel::emballoc;
use bluekernel::{allocator::KernelAllocator, scheduler, thread};

#[cfg(target_pointer_width = "64")]
const EMBALLOC_SIZE: usize = 6 << 20;
#[cfg(target_pointer_width = "32")]
const EMBALLOC_SIZE: usize = 4 << 20;

// TODO: Use libc's malloc/free.
// rust-std has its own allocator and panic_handler.
#[cfg(not(feature = "std"))]
#[global_allocator]
static GLOBAL: KernelAllocator = KernelAllocator;
//static ALLOCATOR: emballoc::Allocator<{ EMBALLOC_SIZE }> = emballoc::Allocator::new();

#[cfg(not(feature = "std"))]
#[panic_handler]
fn oops(info: &core::panic::PanicInfo) -> ! {
    let _ = bluekernel::support::DisableInterruptGuard::new();
    // TODO: Use libc's printf.
    semihosting::println!("Oops: {}", info);
    loop {}
}

#[used]
#[link_section = ".bk_app_array"]
static INIT: extern "C" fn() = init;

extern "C" fn entry() {
    extern "C" {
        // TODO: Support main(argc, argv).
        fn main() -> i32;
    }
    unsafe { main() };
}

extern "C" fn init() {
    let t = thread::Builder::new(thread::Entry::C(entry)).build();
    scheduler::queue_ready_thread(t.state(), t);
}
