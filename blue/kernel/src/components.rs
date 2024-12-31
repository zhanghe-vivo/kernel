use crate::{
    alloc::boxed::Box,
    blue_kconfig::{MAIN_THREAD_PRIORITY, MAIN_THREAD_STACK_SIZE},
    c_str, cpu, idle, kprintf,
    thread::RtThread,
    timer,
};
use blue_arch::arch::Arch;
use core::{pin::Pin, ptr};
use paste::paste;

type InitFn = extern "C" fn() -> i32;

#[cfg(feature = "debugging_init")]
#[repr(C)]
struct RtInitDesc {
    fn_name: *const core::ffi::c_char,
    fn_ptr: InitFn,
}

#[cfg(feature = "debugging_init")]
unsafe impl Sync for RtInitDesc {}

/// convert to string type
macro_rules! level_to_string {
    (level0) => {
        "0"
    };
    (level0_end) => {
        "0.end"
    };
    (level1_end) => {
        "1.end"
    };
    (level6_end) => {
        "6.end"
    };
}

/// initialization export
#[cfg(feature = "debugging_init")]
macro_rules! init_export {
    ($func: ident, $level: ident) => {
        paste! {
            #[allow(non_upper_case_globals)]
            static [<"fuc" $func>]: &str = stringify!($func);
            #[link_section = concat!(".rti_fn.", level_to_string!($level))]
            #[used]
            #[allow(non_upper_case_globals)]
            static [<"rt_init_desc_" $func>]: RtInitDesc = RtInitDesc {
                fn_name: [<"fuc" $func>].as_ptr() as *const core::ffi::c_char,
                fn_ptr: $func,
            };
        }
    };
}

/// initialization export
#[cfg(not(feature = "debugging_init"))]
macro_rules! init_export {
    ($func: ident, $level: expr) => {
        paste! {
            #[allow(non_upper_case_globals)]
            static [<"rti" $level>]: &str = concat!(".rti_fn.", $level);
            #[link_section = concat!(".rti_fn.", level_to_string!($level))]
            #[used]
            #[allow(non_upper_case_globals)]
            static [<"init_fn_" $func>]: InitFn = $func;
        }
    };
}

///initialize some driver and components

#[no_mangle]
pub extern "C" fn rti_start() -> i32 {
    0
}

init_export!(rti_start, level0);

#[no_mangle]
pub extern "C" fn rti_board_start() -> i32 {
    0
}

init_export!(rti_board_start, level0_end);

#[no_mangle]
pub extern "C" fn rti_board_end() -> i32 {
    0
}

init_export!(rti_board_end, level1_end);

#[no_mangle]
pub extern "C" fn rti_end() -> i32 {
    0
}

init_export!(rti_end, level6_end);

///Onboard components initialization.
/// This funtion will be called to complete the initialization of the on-board peripherals.
#[no_mangle]
pub extern "C" fn rt_components_board_init() {
    #[cfg(feature = "debugging_init")]
    {
        let mut desc: *const RtInitDesc = &rt_init_desc_rti_board_start;
        while desc < &rt_init_desc_rti_board_end {
            let desc_ptr = unsafe { &(*desc) };
            let fn_name = desc_ptr.fn_name;
            kprintf!(b"initialize %s", fn_name);
            let result = (desc_ptr.fn_ptr)();
            kprintf!(b":%d done\n", result);
            desc = unsafe { desc.add(1) };
        }
    }
    #[cfg(not(feature = "debugging_init"))]
    {
        let mut fn_ptr = &rt_init_rti_board_start as *const extern "C" fn();
        while fn_ptr < &rt_init_rti_board_end {
            unsafe {
                (fn_ptr as *const extern "C" fn())();
                fn_ptr = fn_ptr.add(1);
            }
        }
    }
}

///kernel components Initialization.
#[no_mangle]
pub extern "C" fn rt_components_init() {
    #[cfg(feature = "debugging_init")]
    {
        let mut desc: *const RtInitDesc = &rt_init_desc_rti_board_end;
        while desc < &rt_init_desc_rti_end {
            let desc_ptr = unsafe { &(*desc) };
            let fn_name = desc_ptr.fn_name;
            kprintf!(b"initialize %s", fn_name);
            let result = (desc_ptr.fn_ptr)();
            kprintf!(b":%d done\n", result);
            desc = unsafe { desc.add(1) };
        }
    }
    #[cfg(not(feature = "debugging_init"))]
    {
        let mut fn_ptr = &rt_init_rti_board_end as *const extern "C" fn();
        while fn_ptr < &rt_init_rti_end {
            unsafe {
                (fn_ptr as *const extern "C" fn())();
                fn_ptr = fn_ptr.add(1);
            }
        }
    }
}

extern "C" {
    pub fn main() -> i32;
    pub fn rt_hw_board_init();
}

#[cfg(not(feature = "heap"))]
#[no_mangle]
static mut MAIN_THREAD_STACK: [u8; MAIN_THREAD_STACK_SIZE] = [0; MAIN_THREAD_STACK_SIZE];

#[cfg(not(feature = "heap"))]
#[no_mangle]
static mut MAIN_THREAD: RtThread = RtThread {};

///The system main thread. In this thread will call the rt_components_init()
#[no_mangle]
pub extern "C" fn main_thread_entry(_parameter: *mut core::ffi::c_void) {
    unsafe {
        rt_components_init();
        #[cfg(feature = "smp")]
        {
            rt_hw_secondary_cpu_up();
        }
        main();
    }
}

///This function will create and start the main thread
#[no_mangle]
pub extern "C" fn rt_application_init() {
    let tid;

    #[cfg(feature = "heap")]
    {
        let thread = RtThread::try_new_in_heap(
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
        let init = RtThread::static_new(
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

///This function will call all levels of initialization functions to complete the initialization of the system, and finally start the scheduler.
#[no_mangle]
pub extern "C" fn kernel_startup() -> ! {
    Arch::disable_interrupts();
    cpu::init_cpus();
    unsafe {
        rt_hw_board_init();
        //TODO: add show version
        // rt_bindings::rt_show_version();
        timer::system_timer_init();
        //TODO: add signal
        #[cfg(feature = "signals")]
        {
            rt_bindings::rt_system_signal_init();
        }
    }
    rt_application_init();
    timer::system_timer_thread_init();
    idle::IdleTheads::init_once();
    #[cfg(feature = "smp")]
    {
        cpu::Cpus::lock_cpus();
    }
    cpu::Cpu::get_current_scheduler().start();

    panic!("!!!system not start!!!");
}
