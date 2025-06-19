use super::{sys_config, systick::Systick, uart};
use crate::{
    allocator,
    arch::Arch,
    devices::{console, virtio},
    early_println,
    error::Error,
    idle::IDLE_HOOK_LIST,
    scheduler::register_reschedule,
};
use core::ptr::addr_of;
use flat_device_tree::Fdt;

#[no_mangle]
extern "C" fn idle_wfi() {
    Arch::wait_for_interrupt();
}

#[no_mangle]
pub extern "C" fn board_init() {
    extern "C" {
        static __heap_start: u64;
    }
    let heap_start = addr_of!(__heap_start) as usize;
    let heap_end = heap_start + sys_config::HEAP_SIZE as usize;

    /* initialize system heap */
    allocator::system_heap_init(heap_start, heap_end);

    // initialize hardware interrupt
    let _ = Systick::init();
    // initialize uart
    match uart::uart_init() {
        Ok(_) => (),
        Err(e) => early_println!("Failed to init uart: {}", Error::from(e)),
    }
    let uart = uart::get_serial0();
    match console::init_console(&uart) {
        Ok(_) => (),
        Err(e) => early_println!("Failed to init console: {}", Error::from(e)),
    }
    // initialize fdt
    // SAFETY: We trust that the FDT pointer we were given is valid, and this is the only time we
    // use it.
    let fdt = unsafe { Fdt::from_ptr(sys_config::DRAM_BASE as *const u8).unwrap() };
    // initialize virtio
    virtio::init_virtio(&fdt);
    // register reschedule
    register_reschedule();

    #[cfg(idle_hook)]
    IDLE_HOOK_LIST.sethook(idle_wfi as *mut _);
}
