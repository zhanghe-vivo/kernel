use crate::syscall_handlers::{dispatch_syscall, Context};
use bluekernel_arch::arm_cortex_m::stack_frame::StackFrame;
use core::arch::naked_asm;

#[no_mangle]
#[naked]
pub unsafe extern "C" fn SVCall() {
    // Avoid clobbering r0 and CSR so that we don't need
    // additional stack ops.
    naked_asm!(
        "ldr   r1, [r0, #24]", //
        "ldrb  r1, [r1, #-2]", // Load SVC number.
        "cbz   r1, 1f",
        "mov.w r0, #-1",
        "bx lr",
        "1:",
        "movs r1, r4",
        "movs r2, r5",
        "movs r3, r7",
        "b SVCall0",
    )
}

#[no_mangle]
extern "C" fn SVCall0(frame: &mut StackFrame, r4: usize, r5: usize, nr: usize) {
    // r7 contains the syscall nr. r0~r6 contain arguments.
    let mut ctx = Context::default();
    ctx.args[0] = frame.r0 as usize;
    ctx.args[1] = frame.r1 as usize;
    ctx.args[2] = frame.r2 as usize;
    ctx.args[3] = frame.r3 as usize;
    ctx.args[4] = r4;
    ctx.args[5] = r5;
    ctx.nr = nr;
    // r0 should contain the return value.
    frame.r0 = dispatch_syscall(&ctx) as u32;
}
