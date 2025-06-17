use crate::{
    errno::SysCallFailed,
    pal::{Pal, Sys},
};
use core::{arch::asm, mem};
use libc::{
    c_double, c_int, c_long, c_uint, c_void, clock_t, clockid_t, time_t, timespec, CLOCK_MONOTONIC,
    CLOCK_REALTIME,
};

pub const CLOCK_PROCESS_CPUTIME_ID: clockid_t = 2;
pub const CLOCKS_PER_SEC: c_long = 1_000_000;

pub struct sigevent;
pub type timer_t = *mut c_void;
pub struct itimerspec {
    pub it_interval: timespec,
    pub it_value: timespec,
}
#[no_mangle]
pub unsafe extern "C" fn clock_gettime(clock_id: clockid_t, tp: *mut timespec) -> c_int {
    match Sys::clock_gettime(clock_id, tp) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/time.html>
#[no_mangle]
pub unsafe extern "C" fn time(tloc: *mut time_t) -> time_t {
    let mut ts = timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    Sys::clock_gettime(CLOCK_REALTIME, &mut ts as *mut timespec);
    if !tloc.is_null() {
        *tloc = ts.tv_sec
    };
    ts.tv_sec
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/clock_getres.html>.
#[no_mangle]
pub unsafe extern "C" fn clock_getres(clock_id: clockid_t, tp: *mut timespec) -> c_int {
    Sys::clock_getres(clock_id, tp).map(|()| 0).syscall_failed()
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/clock_getres.html>.
#[no_mangle]
pub unsafe extern "C" fn clock_settime(clock_id: clockid_t, tp: *const timespec) -> c_int {
    Sys::clock_settime(clock_id, tp)
        .map(|()| 0)
        .syscall_failed()
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/clock_nanosleep.html>.
#[no_mangle]
pub unsafe extern "C" fn clock_nanosleep(
    clock_id: clockid_t,
    flags: c_int,
    rqtp: *const timespec,
    rmtp: *mut timespec,
) -> c_int {
    Sys::clock_nanosleep(clock_id, flags, rqtp, rmtp)
        .map(|()| 0)
        .syscall_failed()
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/nanosleep.html>.
#[no_mangle]
pub unsafe extern "C" fn nanosleep(rqtp: *const timespec, rmtp: *mut timespec) -> c_int {
    Sys::nanosleep(rqtp, rmtp).map(|()| 0).syscall_failed()
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/clock.html>.
#[no_mangle]
pub extern "C" fn clock() -> clock_t {
    let mut ts = mem::MaybeUninit::<timespec>::uninit();

    if unsafe { clock_gettime(CLOCK_PROCESS_CPUTIME_ID, ts.as_mut_ptr()) } != 0 {
        return -1;
    }
    let ts = unsafe { ts.assume_init() };

    let clocks = ts.tv_sec * CLOCKS_PER_SEC + (ts.tv_nsec / (1_000_000_000 / CLOCKS_PER_SEC));
    match clock_t::try_from(clocks) {
        Ok(ok) => ok,
        Err(_err) => -1,
    }
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/difftime.html>.
#[no_mangle]
pub extern "C" fn difftime(time1: time_t, time0: time_t) -> c_double {
    (time1 - time0) as c_double
}

#[no_mangle]
pub extern "C" fn usleep(usec: c_uint) -> c_int {
    let mut rqtp = timespec {
        tv_sec: (usec / 1_000_000) as time_t,
        tv_nsec: ((usec % 1_000_000) * 1000) as c_int,
    };
    let rmtp = core::ptr::null_mut();
    unsafe { nanosleep(&rqtp, rmtp) }
}

#[no_mangle]
pub extern "C" fn msleep(msec: c_uint) -> c_int {
    let mut rqtp = timespec {
        tv_sec: (msec / 1000) as time_t,
        tv_nsec: ((msec % 1000) * 1_000_000) as c_int,
    };
    let rmtp = core::ptr::null_mut();
    unsafe { nanosleep(&rqtp, rmtp) }
}

#[no_mangle]
pub extern "C" fn ssleep(sec: c_uint) -> c_int {
    let mut rqtp = timespec {
        tv_sec: sec as time_t,
        tv_nsec: 0,
    };
    let rmtp = core::ptr::null_mut();
    unsafe { nanosleep(&rqtp, rmtp) }
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/timer_create.html>.
// #[no_mangle]
pub extern "C" fn timer_create(
    clock_id: clockid_t,
    evp: *mut sigevent,
    timerid: *mut timer_t,
) -> c_int {
    unimplemented!();
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/timer_delete.html>.
// #[no_mangle]
pub extern "C" fn timer_delete(timerid: timer_t) -> c_int {
    unimplemented!();
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/timer_getoverrun.html>.
// #[no_mangle]
pub extern "C" fn timer_getoverrun(timerid: timer_t) -> c_int {
    unimplemented!();
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/timer_getoverrun.html>.
// #[no_mangle]
pub extern "C" fn timer_gettime(timerid: timer_t, value: *mut itimerspec) -> c_int {
    unimplemented!();
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/timer_getoverrun.html>.
// #[no_mangle]
pub extern "C" fn timer_settime(
    timerid: timer_t,
    flags: c_int,
    value: *const itimerspec,
    ovalue: *mut itimerspec,
) -> c_int {
    unimplemented!();
}

/// not defined in POSIX, but used in some implementations
#[cfg(target_arch = "arm")]
#[no_mangle]
pub extern "C" fn udelay(usec: c_uint) {
    const CYCLES_PER_USEC: u32 = 1_00; // not exact, this value should get from kernel
    let cycles = usec as u32 * CYCLES_PER_USEC;
    unsafe {
        busy_wait(cycles);
    }
}

#[cfg(target_arch = "arm")]
#[inline(always)]
unsafe fn busy_wait(cycles: u32) {
    let mut count = cycles;
    while count > 0 {
        asm!(
            "subs {0}, {0}, #1",
            inout(reg) count => _,
            options(nomem, nostack, preserves_flags)
        );
    }
}

#[cfg(target_arch = "arm")]
#[no_mangle]
pub extern "C" fn mdelay(msec: c_uint) {
    const CYCLES_PER_MSEC: u32 = 100_000; // not exact, this value should get from kernel
    let cycles = msec as u32 * CYCLES_PER_MSEC;
    unsafe {
        busy_wait(cycles);
    }
}

#[cfg(target_arch = "arm")]
#[no_mangle]
pub extern "C" fn ndelay(nsec: c_uint) {
    udelay(nsec / 1000); // convert nanoseconds to microseconds
}
