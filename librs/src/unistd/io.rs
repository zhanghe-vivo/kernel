use bluekernel_header::syscalls::NR::Write;
use bluekernel_scal::bk_syscall;

#[no_mangle]
#[linkage = "weak"]
pub extern "C" fn write(fd: i32, buf: *const u8, size: usize) -> isize {
    bk_syscall!(Write, fd, buf, size) as isize
}
