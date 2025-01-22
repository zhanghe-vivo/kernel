//! ARM Cortex-M implementation of [`Scheduler`] and context switch.
// Include:
// 1. init_task_stack
//     ```unsafe fn init_task_stack(
//         stack_ptr: *mut usize,
//         stack_bottom: *mut usize,
//         entry: *const usize,
//         arg: *const usize,
//         exit: *const usize,
//     ) -> *mut usize;```
// 2. context_switch_to
//     ```fn context_switch_to(stack_ptr: *const usize) -> !;```
// 3. trigger_switch
//     ```fn trigger_switch();```
// 4. start_switch
//     ```fn start_switch();```

use crate::{
    arch::stack_frame::{StackFrame, StackFrameExtension, StackSettings},
    arm_cortex_m::Arch,
};
use core::{
    arch::{asm, naked_asm},
    mem,
};
use cortex_m::{
    asm,
    peripheral::{scb, scb::VectActive, syst::SystClkSource, SCB},
    register::control::{Fpca, Npriv, Spsel},
    Peripherals,
};

extern "C" {
    static __StackTop: u32;
    static __StackLimit: u32;
}

#[inline]
fn get_exception_lr() -> u32 {
    // EXC_RETURN register bit assignments
    // +---+------+------+-------+-------+
    // | - | S    | FPCA | SPSEL | nPRIV |
    // +---+------+------+-------+-------+
    // S,     [6]   - Secure or Non-secure stack. Indicates whether a Secure or Non-secure stack is used to restore stack frame on exception return.
    //                  0: Non-secure stack used.
    //                  1: Secure stack used.
    //                  Behavior is UNPREDICTABLE if the Security Extension is not implemented and this field is not zero.
    //                  If the Security Extension is not implemented, this bit is RES0.
    // DCRS,  [5]   - Default callee register stacking. Indicates whether the default stacking rules apply, or whether the Additional
    //                  state context, also known as callee registers, are already on the stack.
    //                  0: Stacking of the Additional state context registers skipped.
    //                  1: Default rules for stacking the Additional state context registers followed
    // FType, [4]   - Stack frame type. Indicates whether the stack frame is a standard integer only stack frame or an extended Floating-point stack frame.
    //                  0: Extended stack frame.
    //                  1: Standard stack frame.
    //                  Behavior is UNPREDICTABLE if neither the Floating-point Extension or MVE are implemented and this field is not one.
    //                  If neither the Floating-point Extension or MVE are implemented, this bit is RES1.
    // Mode,   [3]  - Mode. Indicates the Mode that was stacked from.
    //                  0: Handler mode.
    //                  1: Thread mode.
    // SPSEL, [2]   - Stack pointer selection. The value of this bit indicates the transitory value of the CONTROL.
    //                SPSEL bit associated with the Security state of the exception as indicated by EXC_RETURN.ES.
    //                  0: Main stack pointer.
    //                  1: Process stack pointer.
    // Bit [1]      - Reserved, RES0.
    // ES, [0]      - Exception Secure. The security domain the exception was taken to.
    //                  0: Non-secure.
    //                  1: Secure.
    //                  Behavior is UNPREDICTABLE if the Security Extension is not implemented and this field is not zero.
    //                  If the Security Extension is not implemented, this bit is RES0.
    0xFFFFFFFD // thread mode using psp
               // TODO: add trustzone support
}

#[inline]
fn get_control() -> u32 {
    // CONTROL register bit assignments, armv7m only have SPSEL and nPRIV
    // +---------+--------+---------+--------+------+------+-------+-------+
    // | UPAC_EN | PAC_EN | UBTI_EN | BTI_EN | SFPA | FPCA | SPSEL | nPRIV |
    // +---------+--------+---------+--------+------+------+-------+-------+
    // SFPA   [3]     - Indicates that the floating-point registers contain active state that belongs to the Secure state:
    //                   0: The floating-point registers do not contain state that belongs to the Secure state.
    //                   1: The floating-point registers contain state that belongs to the Secure state.
    //                   This bit is not banked between Security states and RAZ/WI from Non-secure state.
    // FPCA   [2]     - Indicates whether floating-point context is active:
    //                   0: No floating-point context active.
    //                   1: Floating-point context active.
    //                   This bit is used to determine whether to preserve floating-point state when processing an exception.
    //                   This bit is not banked between Security states.
    // SPSEL  [1]     - Defines the currently active stack pointer:
    //                   0: MSP is the current stack pointer.
    //                   1: PSP is the current stack pointer.
    //                   In Handler mode, this bit reads as zero and ignores writes. The CortexM33 core updates this bit automatically onexception return.
    //                   This bit is banked between Security states.
    // nPRIV  [0]     - Defines the Thread mode privilege level:
    //                   0: Privileged.
    //                   1: Unprivileged.
    //                   This bit is banked between Security states.
    0x2 // PSP, Thread mode Privileged
}

impl Arch {
    pub unsafe fn init_task_stack(
        stack_ptr: *mut usize,
        stack_bottom: *mut usize,
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

        stack_settings.exception_lr = get_exception_lr();
        stack_settings.control = get_control();

        #[cfg(armv8m)]
        {
            stack_settings.psplim = stack_bottom as u32;
            stack_settings.tz = 0x0;
        }

        stack_ptr.offset(-(stack_offset as isize))

        // TODO: do we need a magic number ?
    }

    #[cfg(any(armv7m, armv7em))]
    pub fn context_switch_to(stack_ptr: *const usize) -> ! {
        // SAFETY: Safe bare metal assembly operations
        unsafe {
            asm!(
                "ldmia r0!, {{r2, r3}}",    // pop exception_lr and control
                "msr   psp, r0",            // set process stack pointer -> task stack
                "msr   control, r3",        // set as thread mode
                "cpsie F",
                "cpsie I",
                "isb",
                "pop   {{r4-r11}}",
                "pop   {{r0-r3,r12,lr}}",   // force function entry
                "pop   {{pc}}",             // 'jump' to the task entry function we put on the stack
                "isb",
                in("r0") stack_ptr as u32,
                options(noreturn),
            )
        }
    }

    #[cfg(armv8m)]
    pub fn context_switch_to(stack_ptr: *const usize) -> ! {
        unsafe {
            asm!(
                "ldmia r0!, {{r1, r2, r3, r4}}",    // pop tz, psplim, exception_lr, control
                "msr   psp, r0",                // set stack pointer
                "msr   psplim, r2",
                "mov   lr, r3",
                "msr   control, r4",            // set as thread mode, no secure
                "cpsie F",
                "cpsie I",
                "isb",
                "pop   {{r4-r11}}",
                "pop   {{r0-r3,r12,lr}}",       // force function entry
                "pop   {{pc}}",                 // 'jump' to the task entry function we put on the stack
                "isb",
                in("r0") stack_ptr as u32,
                options(noreturn),
            )
        }
    }

    #[inline]
    pub fn trigger_switch() {
        SCB::set_pendsv();
        // Barriers are normally not required but do ensure the code is completely
        // within the specified behaviour for the architecture.
        cortex_m::asm::dsb();
        cortex_m::asm::isb();
    }

    pub fn start_switch() {
        // SAFETY: Safe register and assembly operations
        unsafe {
            let mut scb = Peripherals::steal();
            // enable systick counter
            scb.SYST.enable_counter();
            scb.SYST.enable_interrupt();

            // enable PendSV/Systick interrupt on lowest priority
            scb.SCB.set_priority(scb::SystemHandler::PendSV, 0xFF);
            scb.SCB.set_priority(scb::SystemHandler::SysTick, 0xFF);

            // set control register
            let mut control = cortex_m::register::control::read();
            // Check current stack pointer
            if !control.spsel().is_psp() {
                // If using MSP, read MSP and set PSP to the same value
                asm!(
                    "mrs     {tmp}, msp",      // read MSP
                    "msr     psp, {tmp}",      // set to PSP
                    "isb sy",
                    tmp = out(reg) _,
                );

                #[cfg(armv8m)]
                asm!(
                    "mrs     {tmp}, msplim",  // read MSPLIM
                    "msr     psplim, {tmp}",  // set to PSPLIM
                    "isb sy",
                    tmp = out(reg) _,
                );

                // switch to PSP
                control.set_spsel(Spsel::Psp);
                control.set_npriv(Npriv::Privileged);
                #[cfg(has_fpu)]
                {
                    control.set_fpca(Fpca::NotActive);
                }
                asm!(
                    "msr     CONTROL, {0}",
                    "isb",
                    in(reg) control.bits(),
                );
            }

            // reset msp as top
            let stack_top = &__StackTop as *const u32 as u32;
            let stack_limit = &__StackLimit as *const u32 as u32;
            asm!("msr msp, {}", in(reg) stack_top);
            #[cfg(armv8m)]
            {
                asm!("msr msplim, {}", in(reg) stack_limit);
            }

            // Set PendSV and enable exceptions and interrupts
            asm!(
                "ldr     r0, =0xE000ED04", // SCB->ICSR
                "ldr     r1, =(1 << 28)",  // PENDSVSET
                "str     r1, [r0]",
                "cpsie F",
                "cpsie I",
                "isb"
            );
        }
    }
}
