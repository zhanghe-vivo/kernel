#![allow(dead_code)]
use alloc::alloc::{AllocError, LayoutError};
use core::{ffi::CStr, num::TryFromIntError, str::Utf8Error};

pub mod code {
    use libc;
    pub const EOK: super::Error = super::Error(0);
    pub const TRUE: super::Error = super::Error(1);
    pub const FLASE: super::Error = super::Error(0);
    pub const ERROR: super::Error = super::Error(-255);
    pub const ETIMEDOUT: super::Error = super::Error(-libc::ETIMEDOUT);
    pub const ENOSPC: super::Error = super::Error(-libc::ENOSPC);
    pub const ENODATA: super::Error = super::Error(-libc::ENODATA);
    pub const ENOMEM: super::Error = super::Error(-libc::ENOMEM);
    pub const ENOSYS: super::Error = super::Error(-libc::ENOSYS);
    pub const EBUSY: super::Error = super::Error(-libc::EBUSY);
    pub const EIO: super::Error = super::Error(-libc::EIO);
    pub const EINTR: super::Error = super::Error(-libc::EINTR);
    pub const EINVAL: super::Error = super::Error(-libc::EINVAL);
    pub const ENOENT: super::Error = super::Error(-libc::ENOENT);
    pub const ENODEV: super::Error = super::Error(-libc::ENODEV);
    pub const EPERM: super::Error = super::Error(-libc::EPERM);
    pub const EAGAIN: super::Error = super::Error(-libc::EAGAIN);
    pub const EBADF: super::Error = super::Error(-libc::EBADF);
    pub const EEXIST: super::Error = super::Error(-libc::EEXIST);
    pub const ENOTDIR: super::Error = super::Error(-libc::ENOTDIR);
    pub const EISDIR: super::Error = super::Error(-libc::EISDIR);
    pub const ENOTEMPTY: super::Error = super::Error(-libc::ENOTEMPTY);
    pub const ENAMETOOLONG: super::Error = super::Error(-libc::ENAMETOOLONG);
    pub const EACCES: super::Error = super::Error(-libc::EACCES);
    pub const ESPIPE: super::Error = super::Error(-libc::ESPIPE);
    pub const EOVERFLOW: super::Error = super::Error(-libc::EOVERFLOW);
    pub const ELOOP: super::Error = super::Error(-libc::ELOOP);
    pub const EXDEV: super::Error = super::Error(-libc::EXDEV);
}

const UNKNOW_STR: &'static CStr = c"EUNKNOW ";
const EOK_STR: &'static CStr = c"OK  ";
const ERROR_STR: &'static CStr = c"ERROR  ";
const ETIMEDOUT_STR: &'static CStr = c"Timedout  ";
const ENOSPC_STR: &'static CStr = c"No space left on device  ";
const ENODATA_STR: &'static CStr = c"No data available  ";
const ENOMEM_STR: &'static CStr = c"Cannot allocate memory  ";
const ENOSYS_STR: &'static CStr = c"Function not implemented  ";
const EBUSY_STR: &'static CStr = c"Device or resource busy  ";
const EIO_STR: &'static CStr = c"Input/output error  ";
const EINTR_STR: &'static CStr = c"Interrupted system call  ";
const EINVAL_STR: &'static CStr = c"Invalid argument  ";
const ENOENT_STR: &'static CStr = c"No such file or directory  ";
const EPERM_STR: &'static CStr = c"Operation not permitted  ";
const ENODEV_STR: &'static CStr = c"No Such Device  ";
const EAGAIN_STR: &'static CStr = c"Try again  ";
const EBADFD_STR: &'static CStr = c"File descriptor in bad state  ";
const EEXIST_STR: &'static CStr = c"File exists ";
const ENOTDIR_STR: &'static CStr = c"Not a directory ";
const EISDIR_STR: &'static CStr = c"Is a directory ";
const ENOTEMPTY_STR: &'static CStr = c"Directory not empty ";
const ENAMETOOLONG_STR: &'static CStr = c"File name too long";
const ESPIPE_STR: &'static CStr = c"Invalid seek";
const EOVERFLOW_STR: &'static CStr = c"Value too large to be stored in data type";
const ELOOP_STR: &'static CStr = c"Too many symbolic links encountered";
const EXDEV_STR: &'static CStr = c"Cross-device link";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
            &code::ENAMETOOLONG => ENAMETOOLONG_STR,
            &code::ESPIPE => ESPIPE_STR,
            &code::EOVERFLOW => EOVERFLOW_STR,
            &code::ELOOP => ELOOP_STR,
            &code::EXDEV => EXDEV_STR,
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

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Convert CStr to str, fallback to error code if conversion fails
        let err_msg = self.name().to_str().unwrap_or("Unknown error");
        write!(f, "Error({}): {}", self.0, err_msg)
    }
}
