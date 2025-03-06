//! vfs_log.rs

use core::ffi::c_char;

// RT-Thread external functions
extern "C" {
    pub fn rt_kprintf(fmt: *const c_char, ...) -> i32;
}

#[macro_export]
macro_rules! vfslog {
    ($($arg:tt)*) => {{
        let s = alloc::format!("\x1b[34m[blueos-vfs] {}\x1b[0m\n\0",
            alloc::format!($($arg)*));
        unsafe {
            $crate::vfs::vfs_log::rt_kprintf(s.as_ptr() as *const core::ffi::c_char);
        }
    }};
}

pub use vfslog;
