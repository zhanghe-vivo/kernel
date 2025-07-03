use super::{config, uart};
#[cfg(virtio)]
use crate::devices::virtio;
use crate::{
    arch,
    devices::{console, dumb},
    error::Error,
    time,
};
use alloc::sync::Arc;
use blueos_kconfig::NUM_CORES;
#[cfg(virtio)]
use flat_device_tree::Fdt;
pub(crate) fn init() {
    crate::boot::init_runtime();
    unsafe { crate::boot::init_heap() };

    // arch::vector::init();
    unsafe { arch::irq::init(config::GICD as u64, config::GICR as u64, NUM_CORES, false) };

    // time::systick_init(0);
    match uart::uart_init() {
        Ok(_) => (),
        Err(e) => panic!("Failed to init uart: {}", Error::from(e)),
    }
    match console::init_console(dumb::get_serial0().clone()) {
        Ok(_) => (),
        Err(e) => panic!("Failed to init console: {}", Error::from(e)),
    }
    #[cfg(virtio)]
    {
        // initialize fdt
        // SAFETY: We trust that the FDT pointer we were given is valid, and this is the only time we
        // use it.
        let fdt = unsafe { Fdt::from_ptr(config::DRAM_BASE as *const u8).unwrap() };
        // initialize virtio
        virtio::init_virtio(&fdt);
    }
}
