use super::{sys_config, systick::Systick, uart};
use crate::{
    allocator, arch::Arch, drivers::console, early_println, error::Error, idle::IDLE_HOOK_LIST,
};
use core::ptr::addr_of;

extern "C" {
    static __bss_end__: u32;
    static __HeapLimit: u32;
}

#[no_mangle]
extern "C" fn idle_wfi() {
    Arch::wait_for_interrupt();
}

#[no_mangle]
pub extern "C" fn board_init() {
    /* initialize system heap */
    allocator::system_heap_init(
        addr_of!(__bss_end__) as usize,
        addr_of!(__HeapLimit) as usize,
    );
    /* initialize hardware interrupt */
    let _ = Systick::init(sys_config::TICK_PER_SECOND);
    match uart::uart_init() {
        Ok(_) => (),
        Err(e) => early_println!("Failed to init uart: {}", Error::from(e)),
    }
    let uart = uart::get_uart0().clone();
    match console::init_console(uart) {
        Ok(_) => (),
        Err(e) => early_println!("Failed to init console: {}", Error::from(e)),
    }

    #[cfg(os_adapter)]
    {
        extern "C" {
            fn adapter_board_init();

        }
        unsafe { adapter_board_init() };
        unsafe { os_bindings::rt_console_set_device(sys_config::CONSOLE_DEVICE_NAME) };
    }

    #[cfg(idle_hook)]
    IDLE_HOOK_LIST.sethook(idle_wfi as *mut _);
}
