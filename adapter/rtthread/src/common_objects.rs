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

extern crate alloc;
use crate::{os_adapter, rt_def::*};
#[cfg(event_flags)]
use blueos::sync::event_flags::{EventFlags, EventFlagsMode};
use blueos::{error::Error, sync::semaphore::Semaphore, time::timer::Timer, types::Arc};

use delegate::delegate;

// Define the OS adapter types
os_adapter! {
    "sem" => OsSemaphore: Semaphore,
    "timer" => OsTimer: Timer,
}

impl OsSemaphore {
    delegate! {
        to self.inner() {
            pub fn init(&self);
            pub fn count(&self) -> blueos::types::Uint;
            pub fn try_acquire(&self) -> bool;
            pub fn acquire_notimeout(&self) -> bool;
            pub fn acquire_timeout(&self, t: usize) -> bool;
            pub fn acquire(&self, timeout: Option<usize>) -> bool;
            pub fn release(&self);
        }
    }
}

#[cfg(event_flags)]
os_adapter! {
    "evt" => OsEventFlags: EventFlags,
}

#[cfg(event_flags)]
impl OsEventFlags {
    delegate! {
        to self.inner() {
            pub fn init(&self, flags: u32) -> bool;
            pub fn get(&self) -> u32;
            pub fn set(&self, flags: u32) -> Result<u32, Error>;
            pub fn clear(&self, flags: u32) -> u32;
            pub fn wait(&self, flags: u32, mode: EventFlagsMode, timeout: usize) -> Result<u32, Error>;
            pub fn reset(&self);
        }
    }
}
