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

use crate::os_adapter;
use blueos::{
    error::Error,
    sync::event_flags::{EventFlags, EventFlagsMode},
    thread::Thread,
    types::Arc,
};

os_adapter! {
    "th" => OsThread: Thread {
        event_flags: EventFlags,
    }
}

impl OsThread {
    pub fn init_event_flags(&self, flags: u32) {
        self.event_flags.init(flags);
    }

    pub fn set_event_flags(&self, flags: u32) -> Result<u32, Error> {
        self.event_flags.set(flags)
    }

    pub fn clear_event_flags(&self, flags: u32) -> u32 {
        self.event_flags.clear(flags)
    }

    pub fn wait_event_flags(
        &self,
        flags: u32,
        mode: EventFlagsMode,
        timeout: usize,
    ) -> Result<u32, Error> {
        self.event_flags.wait(flags, mode, timeout)
    }

    pub fn get_event_flags(&self) -> u32 {
        self.event_flags.get()
    }
}
