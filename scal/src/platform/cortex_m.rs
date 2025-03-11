// FIXME: Use https://crates.io/crates/syscalls instead.

use core::arch::asm;

#[inline(always)]
pub unsafe fn syscall0(n: usize) -> usize {
    let mut ret: usize;
    asm!(
        "movs {temp}, r7",
        "movs r7, {n}",
        "svc 0",
        "movs r7, {temp}",
        n = in(reg) n,
        temp = out(reg) _,
        lateout("r0") ret,
        options(nostack)
    );
    ret
}
#[inline(always)]
pub unsafe fn syscall1(n: usize, arg1: usize) -> usize {
    let mut ret: usize;
    asm!(
        "movs {temp}, r7",
        "movs r7, {n}",
        "svc 0",
        "movs r7, {temp}",
        n = in(reg) n,
        temp = out(reg) _,
        inlateout("r0") arg1 => ret,
        options(nostack)
    );
    ret
}
