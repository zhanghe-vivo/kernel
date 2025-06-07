use crate::syscall_handlers::{dispatch_syscall, Context};

#[no_mangle]
pub extern "C" fn svcall0(x0: u64, x1: u64, x2: u64, x3: u64, x4: u64, x5: u64, nr: usize) -> u64 {
    // x7 contains the syscall nr. x0~x5 contain arguments.
    let mut ctx = Context::default();
    ctx.args[0] = x0 as usize;
    ctx.args[1] = x1 as usize;
    ctx.args[2] = x2 as usize;
    ctx.args[3] = x3 as usize;
    ctx.args[4] = x4 as usize;
    ctx.args[5] = x5 as usize;
    ctx.nr = nr;
    dispatch_syscall(&ctx) as u64
}
