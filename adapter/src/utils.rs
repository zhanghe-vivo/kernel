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

#![allow(non_upper_case_globals)]

use super::MAX_NAME_LEN;
use blueos::{error::Error, scheduler, thread, types::Arc};
use delegate::delegate;

#[cfg(event_flags)]
use blueos::sync::event_flags::EventFlagsMode;

extern crate alloc;

/// Convert C string to byte array, stopping at first null byte or reaching max_len
pub fn c_name_to_bytes<const LEN: usize>(name_ptr: *const core::ffi::c_char) -> [u8; LEN] {
    debug_assert!(LEN <= 256, "stack-allocated bytes must be less than 256");
    let mut name = [0u8; LEN];
    if name_ptr.is_null() {
        return name;
    }

    let c_str = unsafe { core::ffi::CStr::from_ptr(name_ptr) };
    let bytes = c_str.to_bytes();
    let len = core::cmp::min(bytes.len(), LEN - 1);
    name[..len].copy_from_slice(&bytes[..len]);

    name
}

/// Macro to create OS adapter types with name field and optional additional fields
/// This macro creates new types that wrap Arc<T> with an additional name field and custom fields
#[macro_export]
macro_rules! os_adapter {
    // Basic version without additional fields
    ($($name:ident: $inner_type:ty),* $(,)?) => {
        $(
            static $name: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(1);

            #[derive(Debug)]
            pub struct $name {
                pub inner: Arc<$inner_type>,
                pub name: [u8; $crate::MAX_NAME_LEN],
            }

            impl $name {
                /// Create a new adapter with the given inner object and name
                pub fn new(inner: Arc<$inner_type>, name: [u8; $crate::MAX_NAME_LEN]) -> Self {
                    Self { inner, name }
                }

                /// Create a new adapter with a default name plus counter
                pub fn with_default_name(inner: Arc<$inner_type>) -> Self {
                    let mut name = [0u8; $crate::MAX_NAME_LEN];
                    let name_str = stringify!($name);
                    let counter = $name.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
                    let name_with_counter = alloc::format!("{}{}", name_str, counter);
                    let bytes = name_with_counter.as_bytes();
                    let len = core::cmp::min(bytes.len(), $crate::MAX_NAME_LEN - 1);
                    name[..len].copy_from_slice(&bytes[..len]);
                    Self { inner, name }
                }

                /// Create a new adapter with a custom name from C string
                pub fn with_name(inner: Arc<$inner_type>, name_ptr: *const core::ffi::c_char) -> Self {
                    let name = $crate::utils::c_name_to_bytes::<{ $crate::MAX_NAME_LEN }>(name_ptr);
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
                    let null_pos = self.name.iter().position(|&b| b == 0).unwrap_or($crate::MAX_NAME_LEN);
                    core::str::from_utf8(&self.name[..null_pos]).unwrap_or("")
                }

                /// Set the name of this adapter
                pub fn set_name(&mut self, name: &str) {
                    let bytes = name.as_bytes();
                    let len = core::cmp::min(bytes.len(), $crate::MAX_NAME_LEN - 1);
                    self.name[..len].copy_from_slice(&bytes[..len]);
                    // Null terminate if the string is shorter than MAX_NAME_LEN bytes
                    if len < $crate::MAX_NAME_LEN {
                        self.name[len] = 0;
                    }
                }

                /// Get the raw name bytes
                pub fn name_bytes(&self) -> &[u8; $crate::MAX_NAME_LEN] {
                    &self.name
                }

                /// Clone the inner Arc
                pub fn clone_inner(&self) -> Arc<$inner_type> {
                    Arc::clone(&self.inner)
                }
            }
        )*
    };

    // Extended version with additional fields
    ($name:ident: $inner_type:ty { $($field:ident: $field_type:ty),* $(,)? }) => {
        static $name: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(1);

        #[derive(Debug)]
        pub struct $name {
            pub inner: Arc<$inner_type>,
            pub name: [u8; $crate::MAX_NAME_LEN],

            $(
                pub $field: $field_type,
            )*
        }

        impl $name {
            /// Create a new adapter with the given inner object, name, and additional fields
            pub fn new(inner: Arc<$inner_type>, name: [u8; $crate::MAX_NAME_LEN], $($field: $field_type),*) -> Self {
                Self {
                    inner,
                    name,
                    $($field,)*
                }
            }

            /// Create a new adapter with a default name plus counter and additional fields
            pub fn with_default_name(inner: Arc<$inner_type>) -> Self {
                let mut name = [0u8; $crate::MAX_NAME_LEN];
                let name_str = stringify!($name);
                let counter = $name.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
                let name_with_counter = alloc::format!("{}{}", name_str, counter);
                let bytes = name_with_counter.as_bytes();
                let len = core::cmp::min(bytes.len(), $crate::MAX_NAME_LEN - 1);
                name[..len].copy_from_slice(&bytes[..len]);
                Self {
                    inner,
                    name,
                    $($field: <$field_type>::default(),)*
                }
            }

            /// Create a new adapter with a custom name from C string and additional fields
            pub fn with_name(inner: Arc<$inner_type>, name_ptr: *const core::ffi::c_char) -> Self {
                let name = $crate::utils::c_name_to_bytes::<{ $crate::MAX_NAME_LEN }>(name_ptr);
                Self {
                    inner,
                    name,
                    $($field: <$field_type>::default(),)*
                }
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
                let null_pos = self.name.iter().position(|&b| b == 0).unwrap_or($crate::MAX_NAME_LEN);
                core::str::from_utf8(&self.name[..null_pos]).unwrap_or("")
            }

            /// Set the name of this adapter
            pub fn set_name(&mut self, name: &str) {
                let bytes = name.as_bytes();
                let len = core::cmp::min(bytes.len(), $crate::MAX_NAME_LEN - 1);
                self.name[..len].copy_from_slice(&bytes[..len]);
                // Null terminate if the string is shorter than MAX_NAME_LEN bytes
                if len < $crate::MAX_NAME_LEN {
                    self.name[len] = 0;
                }
            }

            /// Get the raw name bytes
            pub fn name_bytes(&self) -> &[u8; $crate::MAX_NAME_LEN] {
                &self.name
            }

            /// Clone the inner Arc
            pub fn clone_inner(&self) -> Arc<$inner_type> {
                Arc::clone(&self.inner)
            }
        }
    };
}

// Define the OS adapter types
#[cfg(semaphore)]
os_adapter! {
    OsTimer: blueos::time::timer::Timer,
    OsSemaphore: blueos::sync::semaphore::Semaphore,
}

#[cfg(semaphore)]
impl OsSemaphore {
    delegate! {
        to self.inner() {
            pub fn count(&self) -> blueos::types::Int;
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
    OsEventFlags: blueos::sync::event_flags::EventFlags,
}
#[cfg(event_flags)]
impl OsEventFlags {
    delegate! {
        to self.inner() {
            pub fn get(&self) -> u32;
            pub fn set(&self, flags: u32) -> Result<u32, Error>;
            pub fn clear(&self, flags: u32) -> u32;
            pub fn wait(&self, flags: u32, mode: EventFlagsMode, timeout: usize) -> Result<u32, Error>;
        }
    }
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

    use alloc::boxed::Box;

    os_adapter! {
        TestThread: blueos::thread::Thread {
            test_field1: u32,
            test_field2: u32,
        }
    }

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
        let name = create_name_array("TestThread");
        let os_thread = TestThread::new(thread.clone(), name, 1, 2);
        assert_eq!(os_thread.name(), "TestThread");
        scheduler::queue_ready_thread(thread::CREATED, thread);
        scheduler::yield_me(); // to retire the thread
    }

    #[test]
    fn test_os_thread_creation2() {
        let thread = Builder::new(Entry::C(test_thread_entry)).build();
        let os_thread = TestThread::with_default_name(thread.clone());
        assert_eq!(os_thread.name(), "TestThread1");
        scheduler::queue_ready_thread(thread::CREATED, thread);
        scheduler::yield_me(); // to retire the thread
    }
}
