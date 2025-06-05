pub mod components;
pub mod cpuport;
pub mod rt_allocator;
pub mod rt_clock;
#[cfg(condvar)]
pub mod rt_condvar;
pub mod rt_cpu;
pub mod rt_error;
#[cfg(event)]
pub mod rt_event;
#[cfg(idle_hook)]
pub mod rt_idle;
pub mod rt_irq;
pub mod rt_list;
#[cfg(mailbox)]
pub mod rt_mailbox;
#[cfg(messagequeue)]
pub mod rt_message_queue;
#[cfg(mutex)]
pub mod rt_mutex;
pub mod rt_object;
#[cfg(rwlock)]
pub mod rt_rwlock;
pub mod rt_scheduler;
#[cfg(semaphore)]
pub mod rt_semaphore;
pub mod rt_spinlock;
pub mod rt_thread;
pub mod rt_timer;

#[no_mangle]
pub extern "C" fn adapter_board_init() {
    components::rt_components_board_init();
}

#[no_mangle]
pub extern "C" fn adapter_components_init() {
    components::rt_components_init();
}

#[no_mangle]
pub extern "C" fn adapter_console_write_str(s: *const core::ffi::c_char, len: usize) {
    extern "C" {
        fn rt_kputs(str: *const core::ffi::c_char);
    }
    // Create a new slice with null terminator
    let mut buf = [0u8; 128]; // Assuming a reasonable buffer size
    let copy_len = core::cmp::min(len, buf.len() - 1); // Leave space for null terminator
                                                       // Copy the string content
    unsafe {
        core::ptr::copy_nonoverlapping(s, buf.as_mut_ptr() as *mut i8, copy_len);
    }
    // Add \0
    buf[copy_len] = 0;
    unsafe { rt_kputs(buf.as_ptr() as *const core::ffi::c_char) };
}
