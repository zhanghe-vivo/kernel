mod config;
mod handlers;
mod uart;

use crate::{allocator, boot, devices::console, error::Error};
use boot::INIT_BSS_DONE;
pub use uart::get_early_uart;

pub const NUM_CORES: usize = 1;

#[repr(C)]
struct CopyTable {
    src: *const u32,
    dst: *mut u32,
    size: u32,
}

#[repr(C)]
struct ZeroTable {
    dst: *mut u32,
    size: u32,
}

// Copy data from FLASH to RAM.
unsafe fn copy_data() {
    extern "C" {
        static __zero_table_start: ZeroTable;
        static __zero_table_end: ZeroTable;
        static __copy_table_start: CopyTable;
        static __copy_table_end: CopyTable;
    }

    let mut p_table = &__copy_table_start as *const CopyTable;
    while p_table < &__copy_table_end as *const CopyTable {
        let table = &(*p_table);
        for i in 0..table.size {
            core::ptr::write(
                table.dst.add(i as usize),
                core::ptr::read(table.src.add(i as usize)),
            );
        }
        p_table = p_table.add(1);
    }

    let mut p_table = &__zero_table_start as *const ZeroTable;
    while p_table < &__zero_table_end as *const ZeroTable {
        let table = &*p_table;
        for i in 0..table.size {
            core::ptr::write(table.dst.add(i as usize), 0);
        }
        p_table = p_table.add(1);
    }
    INIT_BSS_DONE = true;
}

pub(crate) fn init() {
    unsafe {
        copy_data();
    }
    boot::init_runtime();
    unsafe { boot::init_heap() };
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

pub(crate) fn current_ticks() -> usize {
    0
}
