//! ARM Cortex-M implementation of [`IScheduler`] and context switch.

use core::arch::{asm, naked_asm};
use core::mem;
use cortex_m::peripheral::SCB;

use crate::arch::stack_frame::{StackFrame, StackFrameExtension, StackSettings};
use crate::arch::Arch;
use crate::scheduler::IScheduler;

/// Pendable service call.
///
/// Storing and loading registers in context switch.
///
/// Exception is triggered by `cortex_m::peripheral::SCB::PendSV()`.
#[no_mangle]
#[naked]
pub unsafe extern "C" fn PendSV_Handler() {
    // Based on "Definitive Guide to Cortex-M3/4", p. 349
    #[cfg(has_fpu)]
    unsafe {
        naked_asm!(
            "cpsid   I", // disable interrupt
            "mrs      r1, psp",
            "mov      r3, lr",    // store lr in r2
            "tst      r3, #0x10", // was FPU used?
            "it       eq",
            "vstmdbeq r1!, {{s16-s31}}", // push FPU registers
            //"mrs      r3, control",      // store control register in r3
            "stmdb    r1!, {{r3-r11}}", // push LR, control and remaining registers
            "mov      r0, r1",
            "bl       switch_context_in_irq",
            "ldmia    r0!, {{r3-r11}}",
            //"msr      control, r3",
            //"isb",
            "mov      lr, r3",    // r2 is store as lr
            "tst      lr, #0x10", // was FPU used?
            "it       eq",
            "vldmiaeq r0!, {{s16-s31}}", // pop FPU registers
            "msr      psp, r0",
            "orr      lr, lr, #0x04", // return to thread PSP
            "cpsie    I",
            "bx       lr",
        )
    }

    #[cfg(not(has_fpu))]
    unsafe {
        naked_asm!(
            "cpsid   I", // disable interrupt
            "mrs      r1, psp",
            "mov      r3, lr",
            //"mrs      r3, control",
            "stmdb    r1!, {{r3-r11}}",
            "mov      r0, r1",
            "bl       switch_context_in_irq",
            "ldmia    r0!, {{r3-r11}}",
            //"msr      control, r3",
            //"isb",
            "mov      lr, r3",
            "msr      psp, r0",
            "orr      lr, lr, #0x04",
            "cpsie    I",
            "bx       lr",
        )
    }
}

impl IScheduler for Arch {
    unsafe fn init_task_stack(
        stack_ptr: *mut usize,
        entry: *const usize,
        arg: *const usize,
        exit: *const usize,
    ) -> *mut usize {
        let mut stack_offset = mem::size_of::<StackFrame>() / mem::size_of::<usize>();
        let mut stack_frame: &mut StackFrame =
            mem::transmute(&mut *stack_ptr.offset(-(stack_offset as isize)));
        stack_frame.r0 = arg as u32;
        stack_frame.lr = exit as u32;
        stack_frame.pc = entry as u32;
        stack_frame.xpsr = 0x01000000; // Thumb mode

        // we don't have to initialize r4-r11
        stack_offset += mem::size_of::<StackFrameExtension>() / mem::size_of::<usize>();

        stack_offset += mem::size_of::<StackSettings>() / mem::size_of::<usize>();
        let mut stack_settings: &mut StackSettings =
            mem::transmute(&mut *stack_ptr.offset(-(stack_offset as isize)));
        stack_settings.exception_lr = 0xFFFFFFFD; // thread mode using psp
                                                  // stack_settings.control = 0x2;

        stack_ptr.offset(-(stack_offset as isize))

        // TODO: do we need a magic number ?
    }

    fn context_switch_to(stack_ptr: *const usize) -> ! {
        unsafe {
            asm!(
                "ldmia r0!, {{r2}}",  // pop exception_lr and control
                "mov   r2,  0x2",
                "msr   psp, r0",            // set process stack pointer -> task stack
                "msr   control, r2",  // set as thread mode
                "cpsie F",
                "cpsie I",
                "isb",
                "pop   {{r4-r11}}",
                "pop   {{r0-r3,r12,lr}}",   // force function entry
                "pop   {{pc}}",             // 'jump' to the task entry function we put on the stack
                in("r0") stack_ptr as u32,
                options(noreturn),
            )
        }
    }

    #[inline]
    fn trigger_switch() {
        SCB::set_pendsv();
        // Barriers are normally not required but do ensure the code is completely
        // within the specified behaviour for the architecture.
        cortex_m::asm::dsb();
        cortex_m::asm::isb();
    }
}
