use crate::{c_str, components, cpu, idle, thread::Thread, timer};
use alloc::boxed::Box;
use blue_kconfig::{MAIN_THREAD_PRIORITY, MAIN_THREAD_STACK_SIZE};
use core::{pin::Pin, ptr};

#[cfg(not(feature = "heap"))]
#[no_mangle]
static mut MAIN_THREAD_STACK: [u8; MAIN_THREAD_STACK_SIZE] = [0; MAIN_THREAD_STACK_SIZE];

#[cfg(not(feature = "heap"))]
#[no_mangle]
static mut MAIN_THREAD: Thread = Thread {};

///The system main thread. In this thread will call the components_init()
#[no_mangle]
pub extern "C" fn main_thread_entry(_parameter: *mut core::ffi::c_void) {
    unsafe {
        components::components_init();
        #[cfg(feature = "smp")]
        {
            rt_hw_secondary_cpu_up();
        }

        extern "C" {
            pub fn main() -> i32;
        }
        main();
    }
}

///This function will create and start the main thread
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

        tid = match thread {
            Ok(th) => {
                // need to free by zombie.
                unsafe { Box::leak(Pin::into_inner_unchecked(th)) }
            }
            Err(_) => ptr::null_mut(),
        }
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
    unsafe { (&mut *tid).start() };
}

#[no_mangle]
pub extern "C" fn _startup() -> ! {
    blue_arch::arch::Arch::disable_interrupts();
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
