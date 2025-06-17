use crate::{
    errno::SysCallFailed,
    pal::{Pal, Sys},
};
use libc::{c_char, c_int, pid_t, timespec};

pub struct sched_param {
    pub sched_priority: c_int,
}

pub const SCHED_FIFO: c_int = 0;
pub const SCHED_RR: c_int = 1;
pub const SCHED_OTHER: c_int = 2;

#[no_mangle]
pub extern "C" fn sched_get_priority_max(policy: c_int) -> c_int {
    Sys::sched_get_priority_max(policy)
}
#[no_mangle]
pub extern "C" fn sched_get_priority_min(policy: c_int) -> c_int {
    Sys::sched_get_priority_min(policy)
}

#[no_mangle]
pub unsafe extern "C" fn sched_rr_get_interval(pid: pid_t, time: *mut timespec) -> c_int {
    Sys::sched_rr_get_interval(pid, time)
        .map(|_| 0)
        .syscall_failed()
}

#[no_mangle]
pub unsafe extern "C" fn sched_setscheduler(
    pid: pid_t,
    policy: c_int,
    param: *const sched_param,
) -> c_int {
    // POSIX support scheduler in pthread* functions, just return error value
    -1
}
#[no_mangle]
pub extern "C" fn sched_yield() -> c_int {
    Sys::sched_yield().map(|()| 0).syscall_failed()
}
