pub mod config;
mod handlers;
pub mod uart;

use crate::{
    arch, boot,
    devices::{console, tty::n_tty::Tty, Device},
    error::Error,
    time,
};
use alloc::sync::Arc;
use boot::INIT_BSS_DONE;
use core::ptr::addr_of;

#[repr(C)]
struct CopyTable {
    src: *const u32,
    dest: *mut u32,
    wlen: u32,
}

#[repr(C)]
struct ZeroTable {
    dest: *mut u32,
    wlen: u32,
}

// Copy data from FLASH to RAM.
#[inline(never)]
unsafe fn copy_data() {
    extern "C" {
        static __zero_table_start: ZeroTable;
        static __zero_table_end: ZeroTable;
        static __copy_table_start: CopyTable;
        static __copy_table_end: CopyTable;
    }

    let mut p_table = addr_of!(__copy_table_start);
    while p_table < addr_of!(__copy_table_end) {
        let table = &(*p_table);
        for i in 0..table.wlen {
            core::ptr::write(
                table.dest.add(i as usize),
                core::ptr::read(table.src.add(i as usize)),
            );
        }
        p_table = p_table.offset(1);
    }

    let mut p_table = addr_of!(__zero_table_start);
    while p_table < addr_of!(__zero_table_end) {
        let table = &*p_table;
        for i in 0..table.wlen {
            core::ptr::write(table.dest.add(i as usize), 0);
        }
        p_table = p_table.offset(1);
    }
    INIT_BSS_DONE = true;
}

pub(crate) fn init() {
    unsafe {
        copy_data();
    }
    boot::init_runtime();
    unsafe { boot::init_heap() };
    arch::irq::init();
    time::systick_init(config::SYSTEM_CORE_CLOCK);
    match uart::uart_init() {
        Ok(_) => (),
        Err(e) => panic!("Failed to init uart: {}", Error::from(e)),
    }
    let device: Arc<dyn Device> = if (cfg!(line_discipline)) {
        Tty::init(uart::get_serial0().clone()).clone()
    } else {
        uart::get_serial0().clone()
    };
    match console::init_console(device) {
        Ok(_) => (),
        Err(e) => panic!("Failed to init console: {}", Error::from(e)),
    }
}
