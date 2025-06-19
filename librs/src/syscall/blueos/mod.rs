use super::Syscall;
use crate::{
    c_str::CStr,
    errno::{Errno, Result},
    mqueue::mq_attr,
};
#[allow(unused_imports)]
use bluekernel_header::syscalls::NR::{ClockGetTime, Close, Lseek, Open, Write};
use bluekernel_scal::bk_syscall;
use libc::{
    c_char, c_int, c_uint, c_void, clockid_t, dev_t, mode_t, off_t, size_t, ssize_t, statvfs,
    timespec, utsname,
};

// convert value returned by syscall to user Result.
const ERRNO_MAX: usize = 4095;
pub fn to_result(result: usize) -> Result<usize> {
    if result > ERRNO_MAX.wrapping_neg() {
        Err(Errno(result.wrapping_neg() as _))
    } else {
        Ok(result)
    }
}
pub struct Sys;

impl Syscall for Sys {
    unsafe fn mmap(
        _addr: *mut c_void,
        _len: usize,
        _prot: c_int,
        _flags: c_int,
        _fildes: c_int,
        _off: off_t,
    ) -> Result<*mut c_void> {
        // This is not valid for blueos now
        Err(Errno(-1))
    }

    unsafe fn munmap(_addr: *mut c_void, _len: usize) -> Result<()> {
        // This is not valid for blueos now, do nothing
        Ok(())
    }

    unsafe fn clock_gettime(clk_id: clockid_t, tp: *mut timespec) -> Result<()> {
        match bk_syscall!(ClockGetTime, clk_id, tp) {
            0 => Ok(()),
            _ => Err(Errno(-1)),
        }
    }
    fn write(fildes: c_int, buf: &[u8]) -> Result<usize> {
        to_result(bk_syscall!(Write, fildes, buf.as_ptr() as *const u8, buf.len()) as usize)
    }
    unsafe fn clock_getres(_clk_id: clockid_t, _tp: *mut timespec) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    unsafe fn clock_settime(_clk_id: clockid_t, _tp: *const timespec) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    unsafe fn nanosleep(_rqtp: *const timespec, _rmtp: *mut timespec) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    unsafe fn clock_nanosleep(
        _clk_id: clockid_t,
        _flags: c_int,
        _rqtp: *const timespec,
        _rmtp: *mut timespec,
    ) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn access(_path: CStr, _mode: c_int) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn chdir(_path: CStr) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn chmod(_path: CStr, _mode: mode_t) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn fchmod(_fildes: c_int, _mode: mode_t) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn fdatasync(_fildes: c_int) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    unsafe fn fstat(_fildes: c_int, _buf: *mut c_char) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    unsafe fn fstatvfs(_fildes: c_int, _buf: *mut statvfs) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn fsync(_fildes: c_int) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn ftruncate(_fildes: c_int, _length: off_t) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn dup(_fildes: c_int) -> Result<c_int> {
        // blueos is not valid for this syscall now
        Err(Errno(-1))
    }
    unsafe fn uname(_utsname: *mut utsname) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn open(path: CStr, oflag: c_int, mode: mode_t) -> c_int {
        // blueos is not valid for this syscall now
        bk_syscall!(Open, path.as_ptr(), oflag, mode) as c_int
    }
    fn close(fildes: c_int) -> Result<()> {
        to_result(bk_syscall!(Close, fildes) as usize).map(|_| ())
    }
    unsafe fn statfs(_path: CStr, _buf: *mut c_char) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn link(_path1: CStr, _path2: CStr) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn mkdir(_path: CStr, _mode: mode_t) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn mkfifo(_path: CStr, _mode: mode_t) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn mknod(_path: CStr, _mode: mode_t, _dev: dev_t) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn mknodat(_dir_fildes: c_int, _path: CStr, _mode: mode_t, _dev: dev_t) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn pause() -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn nice(_inc: c_int) -> Result<c_int> {
        // blueos is not valid for this syscall now
        Ok(0)
    }
    fn readlink(_path: CStr, _buf: &mut [u8]) -> Result<usize> {
        // blueos is not valid for this syscall now
        Ok(0)
    }
    fn lseek(fildes: c_int, offset: off_t, whence: c_int) -> off_t {
        // blueos is not valid for this syscall now
        bk_syscall!(Lseek, fildes, offset as usize, whence) as off_t
    }
    fn rename(_oldpath: CStr, _newpath: CStr) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn unlink(_path: CStr) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn symlink(_path1: CStr, _path2: CStr) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn sync() -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn umask(mask: mode_t) -> mode_t {
        // blueos is not valid for this syscall now
        mask
    }
    unsafe fn mq_open(
        _name: *const c_char,
        _oflag: c_int,
        _mode: mode_t,
        _attr: *const mq_attr,
    ) -> Result<c_int> {
        // blueos is not valid for this syscall now
        Err(Errno(-1))
    }
    unsafe fn mq_getsetattr(_mqdes: c_int, _new: *mut mq_attr, _old: *mut mq_attr) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    unsafe fn mq_unlink(_name: *const c_char) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    unsafe fn mq_timedsend(
        _mqdes: c_int,
        _msg_ptr: *const c_char,
        _msg_len: size_t,
        _msg_prio: c_uint,
        _timeout: *const timespec,
    ) -> Result<c_int> {
        // blueos is not valid for this syscall now
        Err(Errno(-1))
    }
    unsafe fn mq_timedreceive(
        _mqdes: c_int,
        _msg_ptr: *mut c_char,
        _msg_len: size_t,
        _msg_prio: *mut c_uint,
        _timeout: *const timespec,
    ) -> Result<ssize_t> {
        // blueos is not valid for this syscall now
        Err(Errno(-1))
    }
    fn sched_yield() -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    unsafe fn sched_rr_get_interval(_pid: c_int, _interval: *mut timespec) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn sched_get_priority_min(_policy: c_int) -> c_int {
        // blueos is not valid for this syscall now
        0
    }
    fn sched_get_priority_max(_policy: c_int) -> c_int {
        // blueos is not valid for this syscall now
        0
    }
    fn pipe2(_fds: &mut [c_int], _flags: c_int) -> Result<()> {
        // blueos is not valid for this syscall now
        Err(Errno(-1))
    }
    fn pread(_fildes: c_int, _buf: &mut [u8], _off: off_t) -> Result<usize> {
        // blueos is not valid for this syscall now
        Err(Errno(-1))
    }
    fn pwrite(_fildes: c_int, _buf: &[u8], _off: off_t) -> Result<usize> {
        // blueos is not valid for this syscall now
        Err(Errno(-1))
    }
}
