use crate::{c_str, components, cpu, idle, thread::Thread, timer};
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
    unsafe {
        let _ = crate::vfs::vfs_api::vfs_init();
        components::components_init();
        #[cfg(feature = "smp")]
        {
            rt_hw_secondary_cpu_up();
        }

        extern "C" {
            pub fn main() -> i32;
            pub fn test_bk_scal();
        }
        test_bk_scal();
        main();
    }
}

/// This function will create and start the main thread.
fn application_init() {
    let tid;

    #[cfg(feature = "heap")]
    {
        let thread = Thread::try_new_in_heap(
            c_str!("main"),
            main_thread_entry,
            ptr::null_mut() as *mut usize,
            MAIN_THREAD_STACK_SIZE as usize,
            MAIN_THREAD_PRIORITY as u8,
            20 as u32,
        );

        tid = thread.map_or(ptr::null_mut(), |ptr| ptr.as_ptr());
    }
    #[cfg(not(feature = "heap"))]
    {
        tid = &MAIN_THREAD;
        let init = Thread::static_new(
            c_str!("main"),
            core::option::Option::Some(main_thread_entry),
            ptr::null_mut() as *mut usize,
            MAIN_THREAD_STACK.as_mut_ptr(),
            MAIN_THREAD_STACK.len(),
            MAIN_THREAD_PRIORITY as u8,
            20 as u32,
        );
        unsafe {
            let _ = init.__pinned_init(tid);
        }
    }

    if unlikely(tid.is_null()) {
        // TODO: Log something since rare event happens.
        return;
    }
    unsafe { (&mut *tid).start() };
}

#[no_mangle]
pub extern "C" fn _startup() -> ! {
    bluekernel_arch::arch::Arch::disable_interrupts();
    cpu::init_cpus();
    unsafe { timer::TIMER_WHEEL.init_once() };
    idle::IdleTheads::init_once();

    // FIXME: board_init is in bsp, causing dependency issues
    unsafe extern "C" {
        unsafe fn board_init();
    }
    unsafe { board_init() };

    timer::system_timer_thread_init();
    application_init();

    #[cfg(feature = "smp")]
    {
        cpu::Cpus::lock_cpus();
    }
    cpu::Cpu::get_current_scheduler().start();

    panic!("!!!system not start!!!");
}
