use crate::{arch::Arch, bsp, c_str, cpu, idle, thread::ThreadBuilder, timer};
use bluekernel_kconfig::{MAIN_THREAD_PRIORITY, MAIN_THREAD_STACK_SIZE};
use core::{intrinsics::unlikely, ptr};

#[cfg(not(feature = "heap"))]
#[no_mangle]
static mut MAIN_THREAD_STACK: [u8; MAIN_THREAD_STACK_SIZE] = [0; MAIN_THREAD_STACK_SIZE];

#[cfg(not(feature = "heap"))]
#[no_mangle]
static mut MAIN_THREAD: Thread = Thread {};

/// The system main thread. In this thread will call the components_init().
#[no_mangle]
pub extern "C" fn main_thread_entry(_parameter: *mut core::ffi::c_void) {
    let _ = crate::vfs::vfs_api::vfs_init();

    #[cfg(feature = "os_adapter")]
    {
        extern "C" {
            fn adapter_components_init();
        }
        unsafe { adapter_components_init() };
    }

    #[cfg(feature = "smp")]
    {
        rt_hw_secondary_cpu_up();
    }

    // The user's main
    extern "C" {
        fn main() -> i32;
    }
    unsafe { main() };
}

/// This function will create and start the main thread.
fn application_init() {
    let tid;

    #[cfg(feature = "heap")]
    {
        let thread = ThreadBuilder::default()
            .name(c_str!("main"))
            .entry_fn(main_thread_entry)
            .arg(ptr::null_mut() as *mut core::ffi::c_void)
            .stack_size(MAIN_THREAD_STACK_SIZE.try_into().unwrap())
            .priority(MAIN_THREAD_PRIORITY.try_into().unwrap())
            .tick(20)
            .build_from_heap();
        tid = thread.map_or(ptr::null_mut(), |ptr| ptr.as_ptr());
    }
    #[cfg(not(feature = "heap"))]
    {
        tid = &MAIN_THREAD;
        let _ = ThreadBuilder::default()
            .static_allocated(unsafe { NonNull::new_unchecked(tid) })
            .name(c_str!("main"))
            .entry_fn(main_thread_entry)
            .arg(ptr::null_mut() as *mut core::ffi::c_void)
            .stack_start(MAIN_THREAD_STACK.as_mut_ptr())
            .stack_size(MAIN_THREAD_STACK.len())
            .priority(MAIN_THREAD_PRIORITY.try_into().unwrap())
            .tick(20)
            .build_from_static_allocation();
    }

    if unlikely(tid.is_null()) {
        // TODO: Log something since rare event happens.
        return;
    }
    unsafe { (&mut *tid).start() };
}

#[no_mangle]
pub extern "C" fn _startup() -> ! {
    Arch::disable_interrupts();
    cpu::init_cpus();
    unsafe { timer::TIMER_WHEEL.init_once() };
    idle::IdleTheads::init_once();
    bsp::init::board_init();
    timer::system_timer_thread_init();
    application_init();

    #[cfg(feature = "smp")]
    {
        cpu::Cpus::lock_cpus();
    }
    cpu::Cpu::get_current_scheduler().start();

    panic!("!!!system not start!!!");
}
