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

use blueos::allocator;
use core::ffi;

#[no_mangle]
pub unsafe extern "C" fn rt_malloc(size: usize) -> *mut ffi::c_void {
    allocator::malloc(size) as *mut ffi::c_void
}

#[no_mangle]
pub unsafe extern "C" fn rt_free(ptr: *mut ffi::c_void) {
    allocator::free(ptr as *mut u8);
}

#[no_mangle]
pub unsafe extern "C" fn rt_realloc(ptr: *mut ffi::c_void, newsize: usize) -> *mut ffi::c_void {
    allocator::realloc(ptr as *mut u8, newsize) as *mut ffi::c_void
}

#[no_mangle]
pub unsafe extern "C" fn rt_calloc(count: usize, size: usize) -> *mut ffi::c_void {
    allocator::calloc(count, size) as *mut ffi::c_void
}

#[no_mangle]
pub unsafe extern "C" fn rt_malloc_align(size: usize, align: usize) -> *mut ffi::c_void {
    allocator::malloc_align(size, align) as *mut ffi::c_void
}

#[no_mangle]
pub unsafe extern "C" fn rt_free_align(ptr: *mut ffi::c_void, align: usize) {
    allocator::free_align(ptr as *mut u8, align);
}

#[no_mangle]
pub extern "C" fn rt_memory_info(total: *mut usize, used: *mut usize, max_used: *mut usize) {
    let memory_info = allocator::memory_info();
    unsafe {
        *total = memory_info.total;
        *used = memory_info.used;
        *max_used = memory_info.max_used;
    }
}
