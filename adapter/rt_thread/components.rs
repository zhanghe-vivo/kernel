use crate::kernel::println;
#[cfg(feature = "debugging_init")]
use core::ffi::CStr;
use paste::paste;

type InitFn = extern "C" fn() -> i32;

#[cfg(feature = "debugging_init")]
#[repr(C)]
struct InitDesc {
    fn_name: *const core::ffi::c_char,
    fn_ptr: InitFn,
}

#[cfg(feature = "debugging_init")]
unsafe impl Sync for InitDesc {}

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
            static [<"fuc" $func>]: &str = concat!(stringify!($func), "\0");
            #[link_section = concat!(".rti_fn.", level_to_string!($level))]
            #[used]
            #[allow(non_upper_case_globals)]
            static [<"init_desc_" $func>]: InitDesc = InitDesc {
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

/// Onboard components initialization.
/// This funtion will be called to complete the initialization of the on-board peripherals.
pub fn rt_components_board_init() {
    #[cfg(feature = "debugging_init")]
    {
        let mut desc: *const InitDesc = &init_desc_rti_board_start;
        while desc < &init_desc_rti_board_end {
            let desc_ptr = unsafe { &(*desc) };
            let fn_name = desc_ptr.fn_name;
            println!("initialize {}", unsafe {
                CStr::from_ptr(fn_name).to_string_lossy()
            });
            let result = (desc_ptr.fn_ptr)();
            println!(":{} done", result);
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
pub fn rt_components_init() {
    #[cfg(feature = "debugging_init")]
    {
        let mut desc: *const InitDesc = &init_desc_rti_board_end;
        while desc < &init_desc_rti_end {
            let desc_ptr = unsafe { &(*desc) };
            let fn_name = desc_ptr.fn_name;
            println!("initialize {}", unsafe {
                CStr::from_ptr(fn_name).to_string_lossy()
            });
            let result = (desc_ptr.fn_ptr)();
            println!(":{} done", result);
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
