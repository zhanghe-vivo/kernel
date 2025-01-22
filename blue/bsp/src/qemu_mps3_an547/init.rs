use super::{sys_config, systick::Systick, uart};
use crate::rt_bindings;
use crate::arch::Arch;
use crate::kernel::{allocator, components};
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
    uart::uart_init();
    components::rt_components_board_init();
    unsafe {
        rt_bindings::rt_console_set_device(sys_config::CONSOLE_DEVICE_NAME);
        rt_bindings::rt_thread_idle_sethook(Some(idle_wfi));
    }
}
