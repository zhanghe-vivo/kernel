use bluekernel_header::syscalls::NR::ClockGetTime;
use bluekernel_scal::bk_syscall;
use libc::{c_int, clockid_t, timespec};

#[no_mangle]
pub unsafe extern "C" fn clock_gettime(clock_id: clockid_t, tp: *mut timespec) -> c_int {
    bk_syscall!(ClockGetTime, clock_id, tp) as c_int
}
