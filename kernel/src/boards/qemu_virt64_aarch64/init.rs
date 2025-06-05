use super::{sys_config, systick::Systick};
use crate::{allocator, arch::Arch, idle::IDLE_HOOK_LIST, scheduler::register_reschedule};
use core::ptr::addr_of;

extern "C" {
    static __heap_start: u64;
}

#[no_mangle]
extern "C" fn idle_wfi() {
    Arch::wait_for_interrupt();
}

#[no_mangle]
pub extern "C" fn board_init() {
    let heap_start = addr_of!(__heap_start) as usize;
    let heap_end = heap_start + sys_config::HEAP_SIZE as usize;
    /* initialize system heap */
    allocator::system_heap_init(heap_start, heap_end);

    /* initialize hardware interrupt */
    Systick::init();
    register_reschedule();

    #[cfg(idle_hook)]
    IDLE_HOOK_LIST.sethook(idle_wfi as *mut _);
}
