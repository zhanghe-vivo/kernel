use crate::{c_str::CStr, mqueue::mq_attr};
use bluekernel_header::syscalls::NR::{ClockGetTime, Close, Lseek, Open, Write};
use bluekernel_scal::bk_syscall;
use libc::{
    c_char, c_int, c_uint, c_void, clockid_t, dev_t, mode_t, off_t, size_t, ssize_t, statvfs,
    timespec, utsname,
};

use super::Syscall;
use crate::errno::{Errno, Result};

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
        addr: *mut c_void,
        len: usize,
        prot: c_int,
        flags: c_int,
        fildes: c_int,
        off: off_t,
    ) -> Result<*mut c_void> {
        // This is not valid for blueos now
        Err(Errno(-1))
    }

    unsafe fn munmap(addr: *mut c_void, len: usize) -> Result<()> {
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
    unsafe fn clock_getres(clk_id: clockid_t, tp: *mut timespec) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    unsafe fn clock_settime(clk_id: clockid_t, tp: *const timespec) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    unsafe fn nanosleep(rqtp: *const timespec, rmtp: *mut timespec) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    unsafe fn clock_nanosleep(
        clk_id: clockid_t,
        flags: c_int,
        rqtp: *const timespec,
        rmtp: *mut timespec,
    ) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn access(path: CStr, mode: c_int) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn chdir(path: CStr) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn chmod(path: CStr, mode: mode_t) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn fchmod(fildes: c_int, mode: mode_t) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn fdatasync(fildes: c_int) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    unsafe fn fstat(fildes: c_int, buf: *mut c_char) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    unsafe fn fstatvfs(fildes: c_int, buf: *mut statvfs) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn fsync(fildes: c_int) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn ftruncate(fildes: c_int, length: off_t) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn dup(fildes: c_int) -> Result<c_int> {
        // blueos is not valid for this syscall now
        Err(Errno(-1))
    }
    unsafe fn uname(utsname: *mut utsname) -> Result<()> {
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
    unsafe fn statfs(path: CStr, buf: *mut c_char) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn link(path1: CStr, path2: CStr) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn mkdir(path: CStr, mode: mode_t) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn mkfifo(path: CStr, mode: mode_t) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn mknod(path: CStr, mode: mode_t, dev: dev_t) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn mknodat(dir_fildes: c_int, path: CStr, mode: mode_t, dev: dev_t) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn pause() -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn nice(inc: c_int) -> Result<c_int> {
        // blueos is not valid for this syscall now
        Ok(0)
    }
    fn readlink(path: CStr, buf: &mut [u8]) -> Result<usize> {
        // blueos is not valid for this syscall now
        Ok(0)
    }
    fn lseek(fildes: c_int, offset: off_t, whence: c_int) -> off_t {
        // blueos is not valid for this syscall now
        bk_syscall!(Lseek, fildes, offset as usize, whence) as off_t
    }
    fn rename(oldpath: CStr, newpath: CStr) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn unlink(path: CStr) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn symlink(path1: CStr, path2: CStr) -> Result<()> {
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
        name: *const c_char,
        oflag: c_int,
        mode: mode_t,
        attr: *const mq_attr,
    ) -> Result<c_int> {
        // blueos is not valid for this syscall now
        Err(Errno(-1))
    }
    unsafe fn mq_getsetattr(mqdes: c_int, new: *mut mq_attr, old: *mut mq_attr) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    unsafe fn mq_unlink(name: *const c_char) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    unsafe fn mq_timedsend(
        mqdes: c_int,
        msg_ptr: *const c_char,
        msg_len: size_t,
        msg_prio: c_uint,
        timeout: *const timespec,
    ) -> Result<c_int> {
        // blueos is not valid for this syscall now
        Err(Errno(-1))
    }
    unsafe fn mq_timedreceive(
        mqdes: c_int,
        msg_ptr: *mut c_char,
        msg_len: size_t,
        msg_prio: *mut c_uint,
        timeout: *const timespec,
    ) -> Result<ssize_t> {
        // blueos is not valid for this syscall now
        Err(Errno(-1))
    }
    fn sched_yield() -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    unsafe fn sched_rr_get_interval(pid: c_int, interval: *mut timespec) -> Result<()> {
        // blueos is not valid for this syscall now
        Ok(())
    }
    fn sched_get_priority_min(policy: c_int) -> c_int {
        // blueos is not valid for this syscall now
        0
    }
    fn sched_get_priority_max(policy: c_int) -> c_int {
        // blueos is not valid for this syscall now
        0
    }
    fn pipe2(fds: &mut [c_int], flags: c_int) -> Result<()> {
        // blueos is not valid for this syscall now
        Err(Errno(-1))
    }
    fn pread(fildes: c_int, buf: &mut [u8], off: off_t) -> Result<usize> {
        // blueos is not valid for this syscall now
        Err(Errno(-1))
    }
    fn pwrite(fildes: c_int, buf: &[u8], off: off_t) -> Result<usize> {
        // blueos is not valid for this syscall now
        Err(Errno(-1))
    }
}
