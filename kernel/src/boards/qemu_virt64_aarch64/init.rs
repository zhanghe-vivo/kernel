use super::{config, uart};
use crate::{arch, devices::console, error::Error, time};
use bluekernel_kconfig::NUM_CORES;

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
    let uart = uart::get_serial0();
    match console::init_console(&uart) {
        Ok(_) => (),
        Err(e) => panic!("Failed to init console: {}", Error::from(e)),
    }
}
