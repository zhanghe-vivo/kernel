use crate::cpu::Cpu;
use alloc::alloc::{AllocError, LayoutError};
use core::{ffi::CStr, num::TryFromIntError, ptr, str::Utf8Error};

pub mod code {
    use crate::klibc;
    pub const EOK: super::Error = super::Error(0);
    pub const TRUE: super::Error = super::Error(1);
    pub const FLASE: super::Error = super::Error(0);
    pub const ERROR: super::Error = super::Error(-255);
    pub const ETIMEDOUT: super::Error = super::Error(-klibc::ETIMEDOUT);
    pub const ENOSPC: super::Error = super::Error(-klibc::ENOSPC);
    pub const ENODATA: super::Error = super::Error(-klibc::ENODATA);
    pub const ENOMEM: super::Error = super::Error(-klibc::ENOMEM);
    pub const ENOSYS: super::Error = super::Error(-klibc::ENOSYS);
    pub const EBUSY: super::Error = super::Error(-klibc::EBUSY);
    pub const EIO: super::Error = super::Error(-klibc::EIO);
    pub const EINTR: super::Error = super::Error(-klibc::EINTR);
    pub const EINVAL: super::Error = super::Error(-klibc::EINVAL);
    pub const ENOENT: super::Error = super::Error(-klibc::ENOENT);
    pub const ENODEV: super::Error = super::Error(-klibc::ENODEV);
    pub const EPERM: super::Error = super::Error(-klibc::EPERM);
}

const EOK_STR: &'static CStr = crate::c_str!("OK      ");
const ERROR_STR: &'static CStr = crate::c_str!("ERROR   ");
const ETIMEDOUT_STR: &'static CStr = crate::c_str!("Timeout ");
const ENOSPC_STR: &'static CStr = crate::c_str!("No space left on device ");
const ENODATA_STR: &'static CStr = crate::c_str!("No data available ");
const ENOMEM_STR: &'static CStr = crate::c_str!("Cannot allocate memory  ");
const ENOSYS_STR: &'static CStr = crate::c_str!("Function not implemented  ");
const EBUSY_STR: &'static CStr = crate::c_str!("Device or resource busy   ");
const EIO_STR: &'static CStr = crate::c_str!("Input/output error     ");
const EINTR_STR: &'static CStr = crate::c_str!("Interrupted system call ");
const EINVAL_STR: &'static CStr = crate::c_str!("Invalid argument  ");
const ENOENT_STR: &'static CStr = crate::c_str!("No such file or directory  ");
const EPERM_STR: &'static CStr = crate::c_str!("Operation not permitted   ");
const ENODEV_STR: &'static CStr = crate::c_str!("No Such Device ");
const UNKNOW_STR: &'static CStr = crate::c_str!("EUNKNOW ");

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
        match self {
            &code::EOK => EOK_STR,
            &code::ERROR => ERROR_STR,
            &code::ETIMEDOUT => ETIMEDOUT_STR,
            &code::ENOSPC => ENOSPC_STR,
            &code::ENODATA => ENODATA_STR,
            &code::ENOMEM => ENOMEM_STR,
            &code::ENOSYS => ENOSYS_STR,
            &code::EBUSY => EBUSY_STR,
            &code::EIO => EIO_STR,
            &code::EINTR => EINTR_STR,
            &code::EINVAL => EINVAL_STR,
            &code::ENOENT => ENOENT_STR,
            &code::EPERM => EPERM_STR,
            &code::ENODEV => ENODEV_STR,
            _ => UNKNOW_STR,
        }
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
    Error(error).name().as_ptr()
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
