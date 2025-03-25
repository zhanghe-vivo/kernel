use bluekernel_header::syscalls::NR::{Echo, Nop};
use bluekernel_scal::bk_syscall;

#[test_case]
fn test_syscalls() {
    assert_eq!(bk_syscall!(Nop), 0);
    for i in 0..1024 {
        assert_eq!(bk_syscall!(Echo, i), i);
    }
}
