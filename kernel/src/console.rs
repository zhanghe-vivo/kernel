// Copyright (c) 2025 vivo Mobile Communication Co., Ltd.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//       http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::devices::console::{get_console, get_early_uart};
use core::{fmt, str};

#[macro_export]
macro_rules! kprintln {
    ($fmt:expr) => ({
        use core::fmt::Write;
        let mut writer = $crate::console::Console {};
        writer.write_fmt(format_args!(concat!($fmt, "\n"))).unwrap();
    });
    ($fmt:expr, $($arg:tt)*) => ({
        use core::fmt::Write;
        let mut writer = $crate::console::Console {};
        writer.write_fmt(format_args!(concat!($fmt, "\n"), $($arg)*)).unwrap();
    });
}

#[macro_export]
macro_rules! kearly_println {
    ($fmt:expr) => ({
        use core::fmt::Write;
        let mut writer = $crate::console::EarlyConsole {};
        writer.write_fmt(format_args!(concat!($fmt, "\n"))).unwrap();
    });
    ($fmt:expr, $($arg:tt)*) => ({
        use core::fmt::Write;
        let mut writer = $crate::console::EarlyConsole {};
        writer.write_fmt(format_args!(concat!($fmt, "\n"), $($arg)*)).unwrap();
    });
}

pub struct Console;
impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let _ = get_console().write(0, s.as_bytes(), true);
        Ok(())
    }
}

pub struct EarlyConsole;
impl fmt::Write for EarlyConsole {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let mut uart = get_early_uart().lock();
        let _ = uart.write_str(s);
        Ok(())
    }
}
