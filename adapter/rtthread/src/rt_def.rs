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

use blueos::error::{code, Error};

// we only re-export the types and defines that are used in the adapter.
pub use rtthread_header::RT_NAME_MAX;
// types
pub use rtthread_header::{
    rt_base_t, rt_device, rt_err_t, rt_int32_t, rt_object, rt_off_t, rt_size_t, rt_uint32_t,
    rt_uint8_t,
};
// class types
pub use rtthread_header::rt_object_class_type_RT_Object_Class_Event;
// IPC
pub use rtthread_header::RT_IPC_CMD_RESET;
// event flags
pub use rtthread_header::{RT_EVENT_FLAG_AND, RT_EVENT_FLAG_CLEAR, RT_EVENT_FLAG_OR};
// errors
pub use rtthread_header::{
    RT_EBUSY, RT_EEMPTY, RT_EFULL, RT_EINTR, RT_EINVAL, RT_EIO, RT_ENOMEM, RT_ENOSYS, RT_EOK,
    RT_ERROR, RT_ETIMEOUT,
};

#[repr(u32)]
#[derive(Debug, Copy, Clone)]
pub enum RtErr {
    Ok = rtthread_header::RT_EOK,
    Error = rtthread_header::RT_ERROR,
    Timeout = rtthread_header::RT_ETIMEOUT,
    Full = rtthread_header::RT_EFULL,
    Empty = rtthread_header::RT_EEMPTY,
    NoMemory = rtthread_header::RT_ENOMEM,
    NoService = rtthread_header::RT_ENOSYS,
    Busy = rtthread_header::RT_EBUSY,
    IO = rtthread_header::RT_EIO,
    Interrupted = rtthread_header::RT_EINTR,
    Invalid = rtthread_header::RT_EINVAL,
}

impl RtErr {
    pub fn as_rt_err(&self) -> rtthread_header::rt_err_t {
        *self as rtthread_header::rt_err_t
    }
}

impl From<Error> for RtErr {
    fn from(err: Error) -> Self {
        match err {
            code::EOK => RtErr::Ok,
            code::ERROR => RtErr::Error,
            code::ETIMEDOUT => RtErr::Timeout,
            code::ENOMEM => RtErr::NoMemory,
            code::ENOSYS => RtErr::NoService,
            code::EBUSY => RtErr::Busy,
            code::EIO => RtErr::IO,
            code::EINTR => RtErr::Interrupted,
            code::EINVAL => RtErr::Invalid,
            _ => RtErr::Error,
        }
    }
}
