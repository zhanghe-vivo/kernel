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

extern crate alloc;
use crate::{os_adapter, rt_def::*};
use blueos::{
    scheduler,
    thread::Thread,
    types::{Arc, ArcInner},
};
use core::{ffi::c_void, ptr::NonNull};

os_adapter! {
    "th" => OsThread: Thread {
        errno: rt_err_t,
    }
}

#[allow(non_camel_case_types)]
#[repr(transparent)]
pub struct rt_thread(ArcInner<OsThread>);

// rt_thread_t rt_thread_self(void);
#[no_mangle]
pub extern "C" fn rt_thread_self() -> *mut OsThread {
    let thread = scheduler::current_thread();
    if let Some(alien_ptr) = thread.get_alien_ptr() {
        alien_ptr.as_ptr() as *mut OsThread
    } else {
        // need to free when thread is retired
        let os_thread = Arc::new(OsThread::new(thread.clone(), 0));
        let res = Arc::into_raw(os_thread) as *mut rt_thread;
        thread
            .lock()
            .set_alien_ptr(NonNull::new(res as *mut c_void).unwrap());
        res as *mut OsThread
    }
}

#[no_mangle]
pub extern "C" fn rt_get_thread_errno(tid: *mut OsThread) -> rt_err_t {
    unsafe { &*(tid as *mut OsThread) }.errno
}

#[no_mangle]
pub extern "C" fn rt_set_thread_errno(tid: *mut OsThread, error: rt_err_t) {
    unsafe { &mut *(tid as *mut OsThread) }.errno = error;
}

#[no_mangle]
pub extern "C" fn rt_get_thread_errno_addr(tid: *mut OsThread) -> *mut rt_err_t {
    unsafe { &mut *(tid as *mut OsThread) }.errno as *mut rt_err_t
}
