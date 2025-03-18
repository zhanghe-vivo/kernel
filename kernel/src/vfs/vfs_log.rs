#[macro_export]
macro_rules! vfslog {
    ($($arg:tt)*) => {{
        crate::println!("\x1b[34m[blueos-vfs] {}\x1b[0m\n", alloc::format!($($arg)*));
    }};
}

pub use vfslog;
