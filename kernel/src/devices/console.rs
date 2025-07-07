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

use super::{dumb, tty::serial::UartOps, Device, DeviceManager};
use crate::sync::SpinLock;
use alloc::{string::String, sync::Arc};
use embedded_io::ErrorKind;
use spin::Once;

static CONSOLE: Once<Arc<dyn Device>> = Once::new();

pub fn init_console(device: Arc<dyn Device>) -> Result<(), ErrorKind> {
    CONSOLE.call_once(|| device.clone());
    DeviceManager::get().register_device(String::from("console"), device.clone())
}

pub fn get_console() -> Arc<dyn Device> {
    CONSOLE.get().unwrap().clone()
}

#[allow(unconditional_recursion)]
pub fn get_early_uart() -> &'static SpinLock<dyn UartOps> {
    get_early_uart()
}
