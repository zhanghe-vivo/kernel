use crate::c_str;
use crate::cpu::Cpus;
use crate::kprintf;
use crate::rt_bindings::*;
use paste::paste;

#[cfg(all(
    not(feature = "RT_MAIN_THREAD_STACK_SIZE"),
    feature = "RT_USING_USER_MAIN"
))]
pub const RT_MAIN_THREAD_STACK_SIZE: u32 = 2048;

#[cfg(all(
    not(feature = "RT_MAIN_THREAD_PRIORITY"),
    feature = "RT_USING_USER_MAIN"
))]
pub const RT_MAIN_THREAD_PRIORITY: u32 = RT_THREAD_PRIORITY_MAX / 3;

#[cfg(feature = "RT_USING_COMPONENTS_INIT")]
type InitFn = extern "C" fn() -> i32;

#[cfg(all(
    feature = "RT_USING_COMPONENTS_INIT",
    feature = "_MSC_VER",
    feature = "RT_DEBUGING_INIT"
))]
#[repr(C)]
struct RtInitDesc {
    level: *const core::ffi::c_char,
    fn_ptr: InitFn,
    fn_name: *const core::ffi::c_char,
}
#[cfg(all(
    feature = "RT_USING_COMPONENTS_INIT",
    feature = "_MSC_VER",
    feature = "RT_DEBUGING_INIT"
))]
unsafe impl Sync for RtInitDesc {}

#[cfg(all(
    feature = "RT_USING_COMPONENTS_INIT",
    feature = "_MSC_VER",
    feature = "RT_DEBUGING_INIT"
))]
macro_rules! init_export {
    ($func: ident, $level: expr) => {
        paste! {
            static [<"rti" $level>]: &str = concat!(".rti_fn.", $level);
            static [<"fuc" $func>]: &str = stringify!($func);
            #[link_section = concat!(".rti_fn.", level_to_string!($level))]
            #[used]
            static [<"rt_init_desc_msc_" $func>]: RtInitDesc = RtInitDesc {
                level: [<"rti" $level>].as_ptr() as *const core::ffi::c_char,
                fn_ptr: $func,
                fn_name: [<"fuc" $func>].as_ptr() as *const core::ffi::c_char,
            };
        }
    };
}

#[cfg(all(
    feature = "RT_USING_COMPONENTS_INIT",
    feature = "_MSC_VER",
    not(feature = "RT_DEBUGING_INIT")
))]
#[repr(C)]
struct RtInitDesc {
    level: *const core::ffi::c_char,
    fn_ptr: InitFn,
}
#[cfg(all(
    feature = "RT_USING_COMPONENTS_INIT",
    feature = "_MSC_VER",
    not(feature = "RT_DEBUGING_INIT")
))]
unsafe impl Sync for RtInitDesc {}

#[cfg(all(
    feature = "RT_USING_COMPONENTS_INIT",
    feature = "_MSC_VER",
    not(feature = "RT_DEBUGING_INIT")
))]
macro_rules! init_export {
    ($func: ident, $level: expr) => {
        paste! {
            static [<"rti" $level>]: &str = concat!(".rti_fn.", $level);
            #[link_section = concat!(".rti_fn.", level_to_string!($level)) ]
            #[used]
            static [<"rt_init_desc_msc_" $func>]: RtInitDesc = RtInitDesc {
                level: [<"rti" $level>].as_ptr() as *const core::ffi::c_char,
                fn_ptr: $func,
            };
        }
    };
}

#[cfg(all(
    feature = "RT_USING_COMPONENTS_INIT",
    feature = "RT_DEBUGING_INIT",
    not(feature = "_MSC_VER")
))]
#[repr(C)]
struct RtInitDesc {
    fn_name: *const core::ffi::c_char,
    fn_ptr: InitFn,
}
#[cfg(all(
    feature = "RT_USING_COMPONENTS_INIT",
    feature = "RT_DEBUGING_INIT",
    not(feature = "_MSC_VER")
))]
unsafe impl Sync for RtInitDesc {}

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

#[cfg(all(
    feature = "RT_USING_COMPONENTS_INIT",
    feature = "RT_DEBUGING_INIT",
    not(feature = "_MSC_VER")
))]
macro_rules! init_export {
    ($func: ident, $level: ident) => {
        paste! {
            static [<"rti" $level>]: &str = concat!(".rti_fn.", level_to_string!($level));
            static [<"fuc" $func>]: &str = stringify!($func);
            #[link_section = concat!(".rti_fn.", level_to_string!($level))]
            #[used]
            static [<"rt_init_desc_" $func>]: RtInitDesc = RtInitDesc {
                fn_name: [<"fuc" $func>].as_ptr() as *const core::ffi::c_char,
                fn_ptr: $func,
            };
        }
    };
}

#[cfg(all(
    feature = "RT_USING_COMPONENTS_INIT",
    not(feature = "RT_DEBUGING_INIT"),
    not(feature = "_MSC_VER")
))]
macro_rules! init_export {
    ($func: ident, $level: expr) => {
        paste! {
            static [<"rti" $level>]: &str = concat!(".rti_fn.", $level);
            #[link_section = concat!(".rti_fn.", level_to_string!($level))]
            #[used]
            static [<"init_fn_" $func>]: InitFn = $func;
        }
    };
}

#[cfg(feature = "RT_USING_COMPONENTS_INIT")]
#[no_mangle]
pub extern "C" fn rti_start() -> i32 {
    0
}
#[cfg(feature = "RT_USING_COMPONENTS_INIT")]
init_export!(rti_start, level0);

#[cfg(feature = "RT_USING_COMPONENTS_INIT")]
#[no_mangle]
pub extern "C" fn rti_board_start() -> i32 {
    0
}
#[cfg(feature = "RT_USING_COMPONENTS_INIT")]
init_export!(rti_board_start, level0_end);

#[cfg(feature = "RT_USING_COMPONENTS_INIT")]
#[no_mangle]
pub extern "C" fn rti_board_end() -> i32 {
    0
}
#[cfg(feature = "RT_USING_COMPONENTS_INIT")]
init_export!(rti_board_end, level1_end);

#[cfg(feature = "RT_USING_COMPONENTS_INIT")]
#[no_mangle]
pub extern "C" fn rti_end() -> i32 {
    0
}
#[cfg(feature = "RT_USING_COMPONENTS_INIT")]
init_export!(rti_end, level6_end);

#[cfg(feature = "RT_USING_COMPONENTS_INIT")]
#[no_mangle]
pub extern "C" fn rt_components_board_init() {
    #[cfg(feature = "RT_DEBUGING_INIT")]
    {
        let mut desc: *const RtInitDesc = &rt_init_desc_rti_board_start;
        while desc < &rt_init_desc_rti_board_end {
            unsafe {
                let fn_name = (*desc).fn_name;
                kprintf!("initialize %s", fn_name);
                let result = ((*desc).fn_ptr)();
                kprintf!(":%d done\n", result);
                desc = desc.add(1);
            }
        }
    }
    #[cfg(not(feature = "RT_DEBUGING_INIT"))]
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

#[cfg(feature = "RT_USING_COMPONENTS_INIT")]
#[no_mangle]
pub extern "C" fn rt_components_init() {
    #[cfg(feature = "RT_DEBUGING_INIT")]
    {
        let mut desc: *const RtInitDesc = &rt_init_desc_rti_board_end;
        while desc < &rt_init_desc_rti_end {
            unsafe {
                let fn_name = (*desc).fn_name;
                kprintf!(b"initialize %s", fn_name);
                let result = ((*desc).fn_ptr)();
                kprintf!(b":%d done\n", result);
                desc = desc.add(1);
            }
        }
    }
    #[cfg(not(feature = "RT_DEBUGING_INIT"))]
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

#[cfg(feature = "RT_USING_USER_MAIN")]
extern "C" {
    pub fn main() -> i32;
    pub fn rt_hw_board_init();
}

#[cfg(all(feature = "RT_USING_USER_MAIN", not(feature = "RT_USING_HEAP")))]
#[no_mangle]
static mut MAIN_THREAD_STACK: [u8; RT_MAIN_THREAD_STACK_SIZE] = [0; RT_MAIN_THREAD_STACK_SIZE];

#[cfg(all(feature = "RT_USING_USER_MAIN", not(feature = "RT_USING_HEAP")))]
#[no_mangle]
static mut MAIN_THREAD: RtThread = RtThread {};

#[cfg(feature = "RT_USING_USER_MAIN")]
#[no_mangle]
pub extern "C" fn main_thread_entry(parameter: *mut core::ffi::c_void) {
    unsafe {
        #[cfg(feature = "RT_USING_COMPONENTS_INIT")]
        {
            rt_components_init();
        }
        #[cfg(feature = "RT_USING_SMP")]
        {
            rt_hw_secondary_cpu_up();
        }
        main();
    }
}

#[cfg(feature = "RT_USING_USER_MAIN")]
#[no_mangle]
pub extern "C" fn rt_application_init() {
    let tid;
    unsafe {
        #[cfg(feature = "RT_USING_HEAP")]
        {
            tid = rt_thread_create(
                c_str!("main").as_ptr() as *const i8,
                core::option::Option::Some(main_thread_entry),
                RT_NULL as *mut core::ffi::c_void,
                RT_MAIN_THREAD_STACK_SIZE,
                RT_MAIN_THREAD_PRIORITY as u8,
                20 as u32,
            );
        }
        #[cfg(not(feature = "RT_USING_HEAP"))]
        {
            tid = &MAIN_THREAD;
            let result: rt_err_t = rt_thread_init(
                tid,
                c_str!("main").as_ptr() as *const i8,
                core::option::Option::Some(main_thread_entry),
                RT_NULL as *mut core::ffi::c_void,
                MAIN_THREAD_STACK.as_mut_ptr(),
                MAIN_THREAD_STACK.len(),
                RT_MAIN_THREAD_PRIORITY as u8,
                20 as u32,
            );
        }
        rt_thread_startup(tid);
    }
}

#[cfg(feature = "RT_USING_USER_MAIN")]
#[no_mangle]
pub extern "C" fn rtthread_startup() -> i32 {
    unsafe {
        rt_hw_local_irq_disable();
        rt_hw_board_init();
        rt_show_version();
        rt_system_timer_init();
        rt_system_scheduler_init();
        #[cfg(feature = "RT_USING_SIGNALS")]
        {
            rt_system_signal_init();
        }
        rt_application_init();
        rt_system_timer_thread_init();
        rt_thread_idle_init();
        #[cfg(feature = "RT_USING_SMP")]
        {
            Cpus::lock_cpus();
        }
        rt_system_scheduler_start();
    }
    0
}
