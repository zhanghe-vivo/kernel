use crate::{irq, rt_bindings, str::CStr};

use alloc::alloc::{AllocError, LayoutError};

use core::convert::From;
use core::fmt;
use core::num::TryFromIntError;
use core::str::Utf8Error;

pub mod code {
    use crate::str::CStr;

    pub const EOK: super::Error = super::Error(-(crate::rt_bindings::RT_EOK as i32));
    pub const ERROR: super::Error = super::Error(-(crate::rt_bindings::RT_ERROR as i32));
    pub const ETIMEOUT: super::Error = super::Error(-(crate::rt_bindings::RT_ETIMEOUT as i32));
    pub const EFULL: super::Error = super::Error(-(crate::rt_bindings::RT_EFULL as i32));
    pub const EEMPTY: super::Error = super::Error(-(crate::rt_bindings::RT_EEMPTY as i32));
    pub const ENOMEM: super::Error = super::Error(-(crate::rt_bindings::RT_ENOMEM as i32));
    pub const ENOSYS: super::Error = super::Error(-(crate::rt_bindings::RT_ENOSYS as i32));
    pub const EBUSY: super::Error = super::Error(-(crate::rt_bindings::RT_EBUSY as i32));
    pub const EIO: super::Error = super::Error(-(crate::rt_bindings::RT_EIO as i32));
    pub const EINTR: super::Error = super::Error(-(crate::rt_bindings::RT_EINTR as i32));
    pub const EINVAL: super::Error = super::Error(-(crate::rt_bindings::RT_EINVAL as i32));
    pub const ENOENT: super::Error = super::Error(-(crate::rt_bindings::RT_ENOENT as i32));
    pub const ENOSPC: super::Error = super::Error(-(crate::rt_bindings::RT_ENOSPC as i32));
    pub const EPERM: super::Error = super::Error(-(crate::rt_bindings::RT_EPERM as i32));
    pub const ETRAP: super::Error = super::Error(-(crate::rt_bindings::RT_ETRAP as i32));

    const EOK_STR: &'static CStr = crate::c_str!("OK      ");
    const ERROR_STR: &'static CStr = crate::c_str!("ERROR   ");
    const ETIMEOUT_STR: &'static CStr = crate::c_str!("ETIMOUT ");
    const EFULL_STR: &'static CStr = crate::c_str!("ERSFULL ");
    const EEMPTY_STR: &'static CStr = crate::c_str!("ERSEPTY ");
    const ENOMEM_STR: &'static CStr = crate::c_str!("ENOMEM  ");
    const ENOSYS_STR: &'static CStr = crate::c_str!("ENOSYS  ");
    const EBUSY_STR: &'static CStr = crate::c_str!("EBUSY   ");
    const EIO_STR: &'static CStr = crate::c_str!("EIO     ");
    const EINTR_STR: &'static CStr = crate::c_str!("EINTRPT ");
    const EINVAL_STR: &'static CStr = crate::c_str!("EINVAL  ");
    const ENOENT_STR: &'static CStr = crate::c_str!("ENOENT  ");
    const ENOSPC_STR: &'static CStr = crate::c_str!("ENOSPC  ");
    const EPERM_STR: &'static CStr = crate::c_str!("EPERM   ");
    const ETRAP_STR: &'static CStr = crate::c_str!("ETRAP   ");
    const UNKNOW_STR: &'static CStr = crate::c_str!("EUNKNOW ");

    pub fn name(errno: core::ffi::c_int) -> &'static CStr {
        match errno.abs() as u32 {
            crate::rt_bindings::RT_EOK => EOK_STR,
            crate::rt_bindings::RT_ERROR => ERROR_STR,
            crate::rt_bindings::RT_ETIMEOUT => ETIMEOUT_STR,
            crate::rt_bindings::RT_EFULL => EFULL_STR,
            crate::rt_bindings::RT_EEMPTY => EEMPTY_STR,
            crate::rt_bindings::RT_ENOMEM => ENOMEM_STR,
            crate::rt_bindings::RT_ENOSYS => ENOSYS_STR,
            crate::rt_bindings::RT_EBUSY => EBUSY_STR,
            crate::rt_bindings::RT_EIO => EIO_STR,
            crate::rt_bindings::RT_EINTR => EINTR_STR,
            crate::rt_bindings::RT_EINVAL => EINVAL_STR,
            crate::rt_bindings::RT_ENOENT => ENOENT_STR,
            crate::rt_bindings::RT_ENOSPC => ENOSPC_STR,
            crate::rt_bindings::RT_EPERM => EPERM_STR,
            crate::rt_bindings::RT_ETRAP => ETRAP_STR,
            _ => UNKNOW_STR,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Error(core::ffi::c_int);
static mut RT_ERRNO: Error = Error(rt_bindings::RT_EOK as i32);

impl Error {
    pub fn from_errno(errno: core::ffi::c_int) -> Error {
        Error(errno)
    }

    pub fn to_errno(self) -> core::ffi::c_int {
        self.0
    }

    pub fn name(&self) -> &'static CStr {
        code::name(-self.0)
    }
}

impl From<AllocError> for Error {
    fn from(_: AllocError) -> Error {
        code::ENOMEM
    }
}

impl From<TryFromIntError> for Error {
    fn from(_: TryFromIntError) -> Error {
        code::EINVAL
    }
}

impl From<Utf8Error> for Error {
    fn from(_: Utf8Error) -> Error {
        code::EINVAL
    }
}

impl From<LayoutError> for Error {
    fn from(_: LayoutError) -> Error {
        code::ENOMEM
    }
}

impl From<core::fmt::Error> for Error {
    fn from(_: core::fmt::Error) -> Error {
        code::EINVAL
    }
}

impl From<core::convert::Infallible> for Error {
    fn from(e: core::convert::Infallible) -> Error {
        match e {}
    }
}

#[no_mangle]
pub unsafe extern "C" fn rt_strerror(error: rt_bindings::rt_err_t) -> *const core::ffi::c_char {
    Error(error).name().as_char_ptr()
}

#[no_mangle]
pub unsafe extern "C" fn rt_get_errno() -> i32 {
    let nest = irq::rt_interrupt_get_nest();
    if nest != 0 {
        return RT_ERRNO.to_errno();
    }

    let tid = rt_bindings::rt_thread_self();
    if tid.is_null() {
        return RT_ERRNO.to_errno();
    }

    (*tid).error
}

#[no_mangle]
pub unsafe extern "C" fn rt_set_errno(error: i32) {
    let nest = irq::rt_interrupt_get_nest();
    if nest != 0 {
        RT_ERRNO = Error::from_errno(error);
        return;
    }

    let tid = rt_bindings::rt_thread_self();
    if tid.is_null() {
        RT_ERRNO = Error::from_errno(error);
        return;
    }

    (*tid).error = error;
}

#[no_mangle]
pub unsafe extern "C" fn _rt_errno() -> *mut i32 {
    let nest = irq::rt_interrupt_get_nest();
    if nest != 0 {
        return &RT_ERRNO.0 as *const i32 as *mut i32;
    }

    let tid = rt_bindings::rt_thread_self();
    if tid.is_null() {
        return &RT_ERRNO.0 as *const i32 as *mut i32;
    }

    &mut (*tid).error
}
