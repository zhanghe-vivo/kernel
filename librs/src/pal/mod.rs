use crate::{c_str::CStr, errno::Result, mqueue::mq_attr};
use libc::{
    c_char, c_int, c_uint, c_void, clockid_t, dev_t, mode_t, off_t, size_t, ssize_t, statvfs,
    timespec, utsname,
};

pub trait Pal {
    unsafe fn mmap(
        addr: *mut c_void,
        len: usize,
        prot: c_int,
        flags: c_int,
        fildes: c_int,
        off: off_t,
    ) -> Result<*mut c_void>;
    unsafe fn munmap(addr: *mut c_void, len: usize) -> Result<()>;
    unsafe fn clock_gettime(clk_id: clockid_t, tp: *mut timespec) -> Result<()>;
    unsafe fn clock_settime(clk_id: clockid_t, tp: *const timespec) -> Result<()>;
    unsafe fn clock_getres(clk_id: clockid_t, tp: *mut timespec) -> Result<()>;
    unsafe fn nanosleep(rqtp: *const timespec, rmtp: *mut timespec) -> Result<()>;
    unsafe fn clock_nanosleep(
        clk_id: clockid_t,
        flags: c_int,
        rqtp: *const timespec,
        rmtp: *mut timespec,
    ) -> Result<()>;
    fn write(fildes: c_int, buf: &[u8]) -> Result<usize>;
    fn close(fildes: c_int) -> Result<()>;
    fn open(path: CStr, oflag: c_int, mode: mode_t) -> c_int;
    fn access(path: CStr, mode: c_int) -> Result<()>;
    fn chdir(path: CStr) -> Result<()>;
    fn chmod(path: CStr, mode: mode_t) -> Result<()>;
    fn fchmod(fildes: c_int, mode: mode_t) -> Result<()>;
    fn fdatasync(fildes: c_int) -> Result<()>;
    unsafe fn fstat(fildes: c_int, buf: *mut c_char) -> Result<()>;
    unsafe fn statfs(path: CStr, buf: *mut c_char) -> Result<()>;
    unsafe fn fstatvfs(fildes: c_int, buf: *mut statvfs) -> Result<()>;
    fn fsync(fildes: c_int) -> Result<()>;
    fn ftruncate(fildes: c_int, length: off_t) -> Result<()>;
    fn dup(fildes: c_int) -> Result<c_int>;
    unsafe fn uname(utsname: *mut utsname) -> Result<()>;
    fn link(path1: CStr, path2: CStr) -> Result<()>;
    fn mkdir(path: CStr, mode: mode_t) -> Result<()>;
    fn mkfifo(path: CStr, mode: mode_t) -> Result<()>;
    fn mknod(path: CStr, mode: mode_t, dev: dev_t) -> Result<()>;
    fn mknodat(dir_fildes: c_int, path: CStr, mode: mode_t, dev: dev_t) -> Result<()>;
    fn pause() -> Result<()>;
    fn nice(inc: c_int) -> Result<c_int>;
    fn readlink(path: CStr, buf: &mut [u8]) -> Result<usize>;
    fn lseek(fildes: c_int, offset: off_t, whence: c_int) -> off_t;
    fn rename(oldpath: CStr, newpath: CStr) -> Result<()>;
    fn umask(mask: mode_t) -> mode_t;
    fn unlink(path: CStr) -> Result<()>;
    fn symlink(path1: CStr, path2: CStr) -> Result<()>;
    fn sync() -> Result<()>;
    unsafe fn mq_open(
        name: *const c_char,
        oflag: c_int,
        mode: mode_t,
        attr: *const mq_attr,
    ) -> Result<c_int>;
    unsafe fn mq_getsetattr(mqdes: c_int, new: *mut mq_attr, old: *mut mq_attr) -> Result<()>;
    unsafe fn mq_unlink(name: *const c_char) -> Result<()>;
    unsafe fn mq_timedsend(
        mqdes: c_int,
        msg_ptr: *const c_char,
        msg_len: size_t,
        msg_prio: c_uint,
        timeout: *const timespec,
    ) -> Result<c_int>;
    unsafe fn mq_timedreceive(
        mqdes: c_int,
        msg_ptr: *mut c_char,
        msg_len: size_t,
        msg_prio: *mut c_uint,
        timeout: *const timespec,
    ) -> Result<ssize_t>;
    unsafe fn sched_rr_get_interval(pid: c_int, interval: *mut timespec) -> Result<()>;
    fn sched_get_priority_min(policy: c_int) -> c_int;
    fn sched_get_priority_max(policy: c_int) -> c_int;
    fn sched_yield() -> Result<()>;
    fn pipe2(fds: &mut [c_int], flags: c_int) -> Result<()>;
    fn pread(fildes: c_int, buf: &mut [u8], off: off_t) -> Result<usize>;
    fn pwrite(fildes: c_int, buf: &[u8], off: off_t) -> Result<usize>;
}

pub use self::sys::Sys;

#[cfg(feature = "usermode")]
#[path = "usermode/mod.rs"]
pub mod sys;

#[cfg(not(feature = "usermode"))]
#[path = "blueos/mod.rs"]
pub mod sys;
