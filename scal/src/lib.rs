#![no_std]

#[cfg(not(direct_syscall_handler))]
pub mod platform;

#[cfg(not(direct_syscall_handler))]
#[macro_export]
macro_rules! bk_syscall {
    ($nr:expr) => {
        unsafe { $crate::platform::syscall0($nr as usize) }
    };
    ($nr:expr, $a1:expr) => {
        unsafe { $crate::platform::syscall1($nr as usize, $a1 as usize) }
    };
    ($nr:expr, $a1:expr, $a2:expr) => {
        unsafe { $crate::platform::syscall2($nr as usize, $a1 as usize, $a2 as usize) }
    };
    ($nr:expr, $a1:expr, $a2:expr, $a3:expr) => {
        unsafe {
            $crate::platfrom::syscall3($nr as usize, $a1 as usize, $a2 as usize, $a3 as usize)
        }
    };
    ($nr:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr) => {
        unsafe {
            $crate::platform::syscall4(
                $nr as usize,
                $a1 as usize,
                $a2 as usize,
                $a3 as usize,
                $a4 as usize,
            )
        }
    };
    ($nr:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr, $a5:expr) => {
        unsafe {
            $crate::platform::syscall5(
                $nr as usize,
                $a1 as usize,
                $a2 as usize,
                $a3 as usize,
                $a4 as usize,
                $a5 as usize,
            )
        }
    };
    ($nr:expr, $a1:expr, $a2:expr, $a3:expr, $a4:expr, $a5:expr, $a6:expr) => {
        unsafe {
            $crate::platform::syscall6(
                $nr as usize,
                $a1 as usize,
                $a2 as usize,
                $a3 as usize,
                $a4 as usize,
                $a5 as usize,
                $a6 as usize,
            )
        }
    };
}

#[cfg(direct_syscall_handler)]
pub use bluekernel::bk_syscall;

// FIXME: This should be removed if we have test framework available.
#[no_mangle]
pub extern "C" fn test_bk_scal() {
    tests::test_syscalls();
}

mod tests {
    use super::bk_syscall;
    use bluekernel_header::syscalls::NR::{Echo, Nop};
    pub(crate) fn test_syscalls() {
        assert_eq!(bk_syscall!(Nop), 0);
        for i in 0..1024 {
            assert_eq!(bk_syscall!(Echo, i), i);
        }
    }
}
