use crate::{cpu::Cpu, str::CStr};

use alloc::alloc::{AllocError, LayoutError};

use core::{num::TryFromIntError, ptr, str::Utf8Error};

pub mod code {
    use crate::str::CStr;

    pub const EOK: super::Error = super::Error(0);
    pub const TRUE: super::Error = super::Error(1);
    pub const FLASE: super::Error = super::Error(0);
    pub const ERROR: super::Error = super::Error(-255);
    pub const ETIMEOUT: super::Error = super::Error(-116);
    pub const EFULL: super::Error = super::Error(-28);
    pub const EEMPTY: super::Error = super::Error(-61);
    pub const ENOMEM: super::Error = super::Error(-12);
    pub const ENOSYS: super::Error = super::Error(-88);
    pub const EBUSY: super::Error = super::Error(-16);
    pub const EIO: super::Error = super::Error(-5);
    pub const EINTR: super::Error = super::Error(-4);
    pub const EINVAL: super::Error = super::Error(-22);
    pub const ENOENT: super::Error = super::Error(-2);
    pub const ENOSPC: super::Error = super::Error(-28);
    pub const EPERM: super::Error = super::Error(-1);
    pub const ETRAP: super::Error = super::Error(-254);

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
    const EPERM_STR: &'static CStr = crate::c_str!("EPERM   ");
    const ETRAP_STR: &'static CStr = crate::c_str!("ETRAP   ");
    const UNKNOW_STR: &'static CStr = crate::c_str!("EUNKNOW ");

    pub fn name(errno: super::Error) -> &'static CStr {
        match errno {
            EOK => EOK_STR,
            ERROR => ERROR_STR,
            ETIMEOUT => ETIMEOUT_STR,
            EFULL => EFULL_STR,
            EEMPTY => EEMPTY_STR,
            ENOMEM => ENOMEM_STR,
            ENOSYS => ENOSYS_STR,
            EBUSY => EBUSY_STR,
            EIO => EIO_STR,
            EINTR => EINTR_STR,
            EINVAL => EINVAL_STR,
            ENOENT => ENOENT_STR,
            EPERM => EPERM_STR,
            ETRAP => ETRAP_STR,
            _ => UNKNOW_STR,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Error(i32);
static mut ERRNO: Error = Error(0);

impl Error {
    pub fn from_errno(errno: i32) -> Error {
        Error(errno)
    }

    pub fn to_errno(self) -> i32 {
        self.0
    }

    pub fn name(&self) -> &'static CStr {
        code::name(Error(-self.0))
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

pub fn strerror(error: i32) -> *const core::ffi::c_char {
    Error(error).name().as_char_ptr()
}

pub unsafe fn get_errno() -> i32 {
    let nest = Cpu::interrupt_nest_load();
    if nest != 0 {
        return ERRNO.to_errno();
    }

    let tid = Cpu::get_current_thread().map_or(ptr::null_mut(), |thread| thread.as_ptr());
    if tid.is_null() {
        return ERRNO.to_errno();
    }

    (*tid).error.to_errno()
}

pub unsafe fn set_errno(error: i32) {
    let nest = Cpu::interrupt_nest_load();
    if nest != 0 {
        ERRNO = Error::from_errno(error);
        return;
    }

    let tid = Cpu::get_current_thread().map_or(ptr::null_mut(), |thread| thread.as_ptr());
    if tid.is_null() {
        ERRNO = Error::from_errno(error);
        return;
    }

    (*tid).error = Error::from_errno(error);
}

pub unsafe fn _errno() -> *mut i32 {
    let nest = Cpu::interrupt_nest_load();
    if nest != 0 {
        return &raw const ERRNO.0 as *const i32 as *mut i32;
    }

    let tid = Cpu::get_current_thread().map_or(ptr::null_mut(), |thread| thread.as_ptr());
    if tid.is_null() {
        return &raw const ERRNO.0 as *const i32 as *mut i32;
    }

    &mut (*tid).error.0
}
