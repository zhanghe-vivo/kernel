use crate::{
    arch::Arch, boards, c_str, cpu, devices, early_println, idle, logger, thread::ThreadBuilder,
    timer,
};
use bluekernel_kconfig::{MAIN_THREAD_PRIORITY, MAIN_THREAD_STACK_SIZE};
use core::{intrinsics::unlikely, ptr};
use log::warn;
#[cfg(not(heap))]
#[no_mangle]
static mut MAIN_THREAD_STACK: [u8; MAIN_THREAD_STACK_SIZE] = [0; MAIN_THREAD_STACK_SIZE];

#[cfg(not(heap))]
#[no_mangle]
static mut MAIN_THREAD: Thread = Thread {};

/// The system main thread. In this thread will call the components_init().
#[no_mangle]
pub extern "C" fn main_thread_entry(_parameter: *mut core::ffi::c_void) {
    match crate::vfs::vfs_api::vfs_init() {
        Ok(_) => (),
        Err(e) => warn!("Failed to init vfs: {}", e),
    }

    #[cfg(os_adapter)]
    {
        extern "C" {
            fn adapter_components_init();
        }
        unsafe { adapter_components_init() };
    }

    #[cfg(smp)]
    {
        rt_hw_secondary_cpu_up();
    }

    #[cfg(test)]
    crate::utest_main();

    #[cfg(posixtestsuite)]
    {
        extern "C" {
            fn start_posix_testsuite();
        }
        unsafe { start_posix_testsuite() };
    }

    #[cfg(std)]
    {
        extern "C" {
            fn start_blueos_posix();
        }
        unsafe { start_blueos_posix() };
    }

    // The user's main
    #[cfg(not(any(test, posixtestsuite, std)))]
    {
        extern "C" {
            fn main() -> i32;
        }
        unsafe { main() };
    }
}

/// This function will create and start the main thread.
fn application_init() {
    let tid;

    #[cfg(heap)]
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
    #[cfg(not(heap))]
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
    let blueos_logo = r#"
=====            ...
===== .*##*=:    #@@+                                    -*#%%%%#+:       .=*%@@%#*-
::::=:+++#@@@*   #@@+                                 .*@@@*+==+%@@@=    +@@@*++*#@@:
  :#@     +@@@   #@@+  ...      ...      .:--:       :@@@+       -@@@*  .@@@-      .
 :@@@    :#@@+   #@@+  %@@-    .@@@   .+@@@%@@@*.    %@@*         :@@@-  %@@@+-.
 -@@@@@@@@@%=    #@@+  %@@-    .@@@  .@@@-   .@@%   .@@@-          @@@*   +%@@@@@%+:
 -@@@::::-*@@@:  #@@+  %@@-    .@@@  +@@@*****%@@:   @@@=          @@@+     .-=*%@@@#
 -@@@      #@@#  #@@+  %@@-    :@@@  *@@#--------    *@@@.        *@@@.          -@@@:
 -@@@    .=@@@=  #@@+  *@@#.  :%@@@  .@@@=.    .      *@@@+-. .:=%@@@:  -@#+:.  .+@@@.
 -@@@@@@@@@@#-   #@@+   *@@@@@@+@@@   .*@@@@@@@@:      :*@@@@@@@@@#=    -*@@@@@@@@@#.
"#;

    Arch::disable_interrupts();
    cpu::init_cpus();

    crate::early_println!("{}", blueos_logo);

    timer::system_timer_init();
    boards::init::board_init();

    match devices::init() {
        Ok(_) => (),
        Err(e) => early_println!("Failed to init drivers: {:?}", e),
    }
    logger::logger_init();
    timer::system_timer_thread_init();
    idle::IdleTheads::init_once();
    application_init();
    #[cfg(smp)]
    {
        cpu::Cpus::lock_cpus();
    }
    cpu::Cpu::get_current_scheduler().start();

    panic!("!!!system not start!!!");
}
