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

use blueos::types::Arc;

/// Macro to create OS adapter types with name field
/// This macro creates new types that wrap Arc<T> with an additional name field
macro_rules! os_adapter {
    ($($name:ident: $inner_type:ty),* $(,)?) => {
        $(
            #[derive(Debug)]
            pub struct $name {
                pub name: [u8; 16],
                pub inner: Arc<$inner_type>,
            }

            impl $name {
                /// Create a new adapter with the given inner object and name
                pub fn new(inner: Arc<$inner_type>, name: [u8; 16]) -> Self {
                    Self { inner, name }
                }

                /// Create a new adapter with a default name
                pub fn with_default_name(inner: Arc<$inner_type>) -> Self {
                    let mut name = [0u8; 16];
                    let name_str = stringify!($name);
                    let bytes = name_str.as_bytes();
                    let len = core::cmp::min(bytes.len(), 15);
                    name[..len].copy_from_slice(&bytes[..len]);
                    Self { inner, name }
                }

                /// Get a reference to the inner object
                pub fn inner(&self) -> &Arc<$inner_type> {
                    &self.inner
                }

                /// Get a mutable reference to the inner object
                pub fn inner_mut(&mut self) -> &mut Arc<$inner_type> {
                    &mut self.inner
                }

                /// Get the name of this adapter as a string slice
                pub fn name(&self) -> &str {
                    // Find the null terminator or use the full array
                    let null_pos = self.name.iter().position(|&b| b == 0).unwrap_or(16);
                    core::str::from_utf8(&self.name[..null_pos]).unwrap_or("")
                }

                /// Set the name of this adapter
                pub fn set_name(&mut self, name: &str) {
                    let bytes = name.as_bytes();
                    let len = core::cmp::min(bytes.len(), 15);
                    self.name[..len].copy_from_slice(&bytes[..len]);
                    // Null terminate if the string is shorter than 16 bytes
                    if len < 16 {
                        self.name[len] = 0;
                    }
                }

                /// Get the raw name bytes
                pub fn name_bytes(&self) -> &[u8; 16] {
                    &self.name
                }

                /// Clone the inner Arc
                pub fn clone_inner(&self) -> Arc<$inner_type> {
                    Arc::clone(&self.inner)
                }
            }
        )*
    };
}

// Define the OS adapter types
os_adapter! {
    OsThread: blueos::thread::Thread,
    OsTimer: blueos::time::timer::Timer,
    OsSemaphore: blueos::sync::semaphore::Semaphore,
}

#[cfg(event_flags)]
os_adapter! {
    OsEventFlags: blueos::sync::event_flags::EventFlags,
}

#[cfg(test)]
mod tests {
    use super::*;
    use blueos::{
        sync::semaphore::Semaphore,
        thread::{Builder, Entry},
        time::timer::Timer,
    };
    use blueos_test_macro::test;

    extern crate alloc;
    use alloc::boxed::Box;

    /// Helper function to create a name array from a string
    fn create_name_array(name: &str) -> [u8; 16] {
        let mut name_array = [0u8; 16];
        let bytes = name.as_bytes();
        let len = core::cmp::min(bytes.len(), 15);
        name_array[..len].copy_from_slice(&bytes[..len]);
        name_array
    }

    extern "C" fn test_thread_entry() {
        // do nothing
    }

    #[test]
    fn test_os_thread_creation() {
        let thread = Builder::new(Entry::C(test_thread_entry)).build();
        let name = create_name_array("test_thread");
        let os_thread = OsThread::new(thread, name);
        assert_eq!(os_thread.name(), "test_thread");
    }

    #[test]
    fn test_os_timer_creation() {
        let timer = Timer::new_hard_oneshot(1000, Box::new(|| {}));
        let os_timer = OsTimer::with_default_name(timer);
        assert_eq!(os_timer.name(), "OsTimer");
    }

    #[test]
    fn test_os_semaphore_creation() {
        let semaphore = Arc::new(Semaphore::new(1));
        let os_semaphore = OsSemaphore::with_default_name(semaphore);
        assert_eq!(os_semaphore.name(), "OsSemaphore");
    }

    #[cfg(event_flags)]
    #[test]
    fn test_os_event_flags_creation() {
        let event_flags = Arc::new(blueos::sync::event_flags::EventFlags::const_new());
        let os_event_flags = OsEventFlags::with_default_name(event_flags);
        assert_eq!(os_event_flags.name(), "OsEventFlags");
    }
}
