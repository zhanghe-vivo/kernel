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
use crate::rt_def::*;
use blueos::types::Arc;
use core::ffi::c_char;

extern "C" {
    fn rt_object_get_name(obj: *const rt_object, name: *mut c_char, len: rt_uint8_t) -> rt_err_t;
}

/// Macro to create OS adapter types with name field and optional additional fields
/// This macro creates new types that wrap Arc<T> with an additional name field and custom fields
#[macro_export]
macro_rules! os_adapter {
    // Basic version with required name prefix (no additional fields)
    // Syntax: (prefix1 "name1" -> type1: inner_type1, prefix2 "name2" -> type2: inner_type2, ...)
    ($($prefix:expr => $name:ident: $inner_type:ty),* $(,)?) => {
        $(
            #[repr(C)]
            #[derive(Debug)]
            pub struct $name {
                pub obj: rt_object,
                pub inner: Arc<$inner_type>,
            }

            impl $name {
                /// Create a new adapter with the given inner object and name
                /// must call rt_object_init before use
                pub fn new(inner: Arc<$inner_type>) -> Self {
                    Self { obj: rt_object::default(), inner }
                }

                /// Get a reference to the inner object
                pub fn inner(&self) -> &Arc<$inner_type> {
                    &self.inner
                }

                #[cfg(test)]
                pub fn name(&self) -> alloc::string::String {
                    use alloc::string::ToString;
                    extern "C" {
                        fn rt_object_get_name(obj: *const rt_object, name: *mut core::ffi::c_char, len: rt_uint8_t) -> rt_err_t;
                    }
                    let mut name = [0; RT_NAME_MAX as usize];
                    unsafe {
                        rt_object_get_name(&self.obj, name.as_mut_ptr() as *mut core::ffi::c_char, RT_NAME_MAX as rt_uint8_t);
                        core::ffi::CStr::from_ptr(name.as_ptr() as *const core::ffi::c_char)
                            .to_string_lossy()
                            .to_string()
                    }
                }
            }
        )*
    };

    // Extended version with required name prefix and additional fields
    // Syntax: prefix "name" -> type_name: inner_type { fields }
    ($prefix:expr => $name:ident: $inner_type:ty { $($field:ident: $field_type:ty),* $(,)? }) => {
        #[repr(C)]
        #[derive(Debug)]
        pub struct $name {
            pub obj: rt_object,
            pub inner: Arc<$inner_type>,

            $(
                pub $field: $field_type,
            )*
        }

        impl $name {
            /// Create a new adapter with the given inner object, and additional fields
            pub fn new(inner: Arc<$inner_type>, $($field: $field_type),*) -> Self {
                Self {
                    obj: rt_object::default(),
                    inner,
                    $($field,)*
                }
            }

            /// Get a reference to the inner object
            pub fn inner(&self) -> &Arc<$inner_type> {
                &self.inner
            }

            #[cfg(test)]
            pub fn name(&self) -> alloc::string::String {
                use alloc::string::ToString;
                extern "C" {
                    fn rt_object_get_name(obj: *const rt_object, name: *mut core::ffi::c_char, len: rt_uint8_t) -> rt_err_t;
                }
                let mut name = [0; RT_NAME_MAX as usize];
                unsafe {
                    rt_object_get_name(&self.obj, name.as_mut_ptr() as *mut core::ffi::c_char, RT_NAME_MAX as rt_uint8_t);
                    core::ffi::CStr::from_ptr(name.as_ptr() as *const core::ffi::c_char)
                        .to_string_lossy()
                        .to_string()
                }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use blueos::types::Arc;
    use blueos_test_macro::test;

    #[derive(Debug)]
    pub struct TestField {
        field1: u32,
    }

    os_adapter! {
        "th" => OsTest: TestField {
            field2: u32,
            field3: u32,
        }
    }

    #[test]
    fn test_os_adapter() {
        let inner = Arc::new(TestField { field1: 1 });
        let field = OsTest::new(inner, 2, 3);
        assert_eq!(field.inner().field1, 1);
        assert_eq!(field.field2, 2);
        assert_eq!(field.field3, 3);
    }
}
