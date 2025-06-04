#![allow(dead_code)]
#[cfg(not(cortex_a))]
use crate::drivers::console::{get_console, get_early_uart};
use core::{fmt, str};

#[macro_export]
macro_rules! println {
    ($fmt:expr) => ({
        #[cfg(cortex_a)]
        {
            semihosting::println!($fmt);
        }
        #[cfg(not(cortex_a))]
        {
            use core::fmt::Write;
            let mut writer = $crate::console::Console {};
            writer.write_fmt(format_args!(concat!($fmt, "\n"))).unwrap();
        }
    });
    ($fmt:expr, $($arg:tt)*) => ({
        #[cfg(cortex_a)]
        {
            semihosting::println!("{}", format_args!($fmt, $($arg)*));
        }
        #[cfg(not(cortex_a))]
        {
            use core::fmt::Write;
            let mut writer = $crate::console::Console {};
            writer.write_fmt(format_args!(concat!($fmt, "\n"), $($arg)*)).unwrap();
        }
    });
}

#[macro_export]
macro_rules! early_println {
    ($fmt:expr) => ({
        #[cfg(cortex_a)]
        {
            semihosting::println!("{}", format_args!($fmt, $($arg)*));
        }
        #[cfg(not(cortex_a))]
        {
            use core::fmt::Write;
            let mut writer = $crate::console::EarlyConsole {};
            writer.write_fmt(format_args!(concat!($fmt, "\n"))).unwrap();
        }
    });
    ($fmt:expr, $($arg:tt)*) => ({
        #[cfg(cortex_a)]
        {
            semihosting::println!("{}", format_args!($fmt, $($arg)*));
        }
        #[cfg(not(cortex_a))]
        {
            use core::fmt::Write;
            let mut writer = $crate::console::EarlyConsole {};
            writer.write_fmt(format_args!(concat!($fmt, "\n"), $($arg)*)).unwrap();
        }
    });
}

#[cfg(not(cortex_a))]
pub struct Console;

#[cfg(not(cortex_a))]
impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let _ = get_console().write(0, s.as_bytes(), true);
        Ok(())
    }
}

#[cfg(not(cortex_a))]
pub struct EarlyConsole;

#[cfg(not(cortex_a))]
impl fmt::Write for EarlyConsole {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let mut uart = get_early_uart().lock();
        let _ = uart.write_str(s);
        Ok(())
    }
}
