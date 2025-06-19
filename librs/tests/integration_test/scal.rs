use crate::println;
use bluekernel_scal::bk_syscall;
use bluekernel_test_macro::test;

#[allow(unused_imports)]
pub use bluekernel_header::syscalls::NR::{Echo, Nop};
#[test]
fn test_syscalls() {
    assert_eq!(bk_syscall!(Nop), 0);
    for i in 0..1024 {
        assert_eq!(bk_syscall!(Echo, i), i);
    }
}
