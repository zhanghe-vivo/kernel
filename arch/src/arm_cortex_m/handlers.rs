use crate::arch::{reset_handler_inner, Vector};
use core::arch::naked_asm;
use cortex_m::asm;

#[link_section = ".text.vector_handlers"]
#[linkage = "weak"]
#[no_mangle]
pub unsafe extern "C" fn Reset_Handler() {
    extern "C" {
        static __StackTop: u32;
    }

    asm::bootstrap(&__StackTop as *const _, reset_handler_inner as *const u32)
}

/// Pendable service call.
///
/// Storing and loading registers in context switch.
///
/// Exception is triggered by `cortex_m::peripheral::SCB::PendSV()`.
#[cfg(all(any(armv7m, armv7em), not(has_fpu)))]
#[link_section = ".text.vector_handlers"]
#[no_mangle]
#[naked]
pub unsafe extern "C" fn PendSV_Handler() {
    // SAFETY: Safe bare metal assembly operations
    unsafe {
        naked_asm!(
            "cpsid   I", // disable interrupt
            "mrs      r0, psp",
            "mov      r2, lr",
            "mrs      r3, control",
            "stmdb    r0!, {{r2-r11}}",
            "bl       switch_context_in_irq",
            "ldmia    r0!, {{r2-r11}}",
            "msr      control, r3",
            "isb",
            "mov      lr, r2",
            "msr      psp, r0",
            "orr      lr, lr, #0x04",
            "cpsie    I",
            "bx       lr",
        )
    }
}

#[cfg(all(any(armv7m, armv7em), has_fpu))]
#[link_section = ".text.vector_handlers"]
#[no_mangle]
#[naked]
pub unsafe extern "C" fn PendSV_Handler() {
    unsafe {
        naked_asm!(
            "cpsid   I", // disable interrupt
            "mrs      r0, psp",
            "mov      r2, lr",
            "mrs      r3, control",
            "tst      r2, #0x10",
            "it       eq",
            "vstmdbeq r0!, {{s16-s31}}", // push FPU registers
            "stmdb    r0!, {{r2-r11}}",
            "bl       switch_context_in_irq",
            "ldmia    r0!, {{r2-r11}}",
            "msr      control, r3",
            "mov      lr, r2",
            "tst      lr, #0x10",
            "it       eq",
            "vldmiaeq r0!, {{s16-s31}}", // pop FPU registers
            "msr      psp, r0",
            "cpsie    I",
            "bx       lr",
        )
    }
}

#[cfg(all(armv8m, not(has_fpu)))]
#[link_section = ".text.vector_handlers"]
#[no_mangle]
#[naked]
pub unsafe extern "C" fn PendSV_Handler() {
    unsafe {
        naked_asm!(
            "cpsid    I",
            "mrs      r0, psp",
            "stmdb    r0!, {{r4-r11}}",
            "mov      r1, #0",
            "mrs      r2, psplim",
            "mov      r3, lr",
            "mrs      r4, control",
            "stmdb    r0!, {{r1-r4}}",
            "bl       switch_context_in_irq",
            "ldmia    r0!, {{r1-r4}}",
            "msr      control, r4",
            "msr      psplim, r2",
            "mov      lr, r3",
            "ldmia    r0!, {{r4-r11}}",
            "msr      psp, r0",
            "cpsie    I",
            "bx       lr",
        )
    }
}
#[cfg(all(armv8m, has_fpu))]
#[link_section = ".text.vector_handlers"]
#[no_mangle]
#[naked]
pub unsafe extern "C" fn PendSV_Handler() {
    unsafe {
        naked_asm!(
            "cpsid    I",
            "mrs      r0, psp",
            // test and push FPU registers
            "tst      lr, #0x10",
            "it       eq",
            "vstmdbeq r0!, {{s16-s31}}",
            // push general registers
            "stmdb    r0!, {{r4-r11}}",
            "mov      r1, #0", // no tz context supported yet. reserve for future.
            "mrs      r2, psplim",
            "mov      r3, lr",
            "mrs      r4, control",
            "stmdb    r0!, {{r1-r4}}",
            "bl       switch_context_in_irq",
            "ldmia    r0!, {{r1-r4}}",
            // no tz context supported yet.
            "msr      control, r4",
            "msr      psplim, r2",
            "mov      lr, r3",
            "ldmia    r0!, {{r4-r11}}",
            // test and pop FPU registers
            "tst      lr, #0x10",
            "it       eq",
            "vldmiaeq r0!, {{s16-s31}}",
            "msr      psp, r0",
            "cpsie    I",
            "bx       lr",
        )
    }
}

#[link_section = ".text.vector_handlers"]
#[linkage = "weak"]
#[no_mangle]
pub unsafe extern "C" fn Default_Handler() {
    #[allow(clippy::empty_loop)]
    loop {}
}

#[link_section = ".text.vector_handlers"]
#[linkage = "weak"]
#[no_mangle]
pub unsafe extern "C" fn NMI_Handler() {
    Default_Handler();
}

#[cfg(any(armv7m, armv7em))]
#[link_section = ".text.vector_handlers"]
#[no_mangle]
#[naked]
pub unsafe extern "C" fn HardFault_Handler() {
    // SAFETY: This is a hardware exception handler, using naked assembly is safe.
    unsafe {
        naked_asm!(
            "mrs      r0, msp",   // get fault context from handler.
            "tst      lr, #0x04", // if(!EXC_RETURN[2])
            "beq      1f",
            "mrs      r0, psp",         // get fault context from thread.
            "1: mov   r2, lr",          // store lr in r2
            "mrs      r3, control",     // store control register in r3
            "stmdb    r0!, {{r2-r11}}", // push LR, control and remaining registers
            "tst      lr, #0x04",       // if(!EXC_RETURN[2])
            "beq      2f",
            "msr      psp, r0", // update stack pointer to PSP.
            "b        3f",
            "2: msr   msp, r0", // update stack pointer to MSP.
            "3: push  {{lr}}",  // save origin lr
            "bl      HardFault",
            "pop     {{lr}}",
            "bx      lr",
        )
    }
}

#[cfg(armv8m)]
#[link_section = ".text.vector_handlers"]
#[no_mangle]
#[naked]
pub unsafe extern "C" fn HardFault_Handler() {
    unsafe {
        naked_asm!(
            // 1. Determine which stack pointer was used
            "tst     lr, #0x04",
            "ite     eq",
            "mrseq   r0, msp",
            "mrsne   r0, psp",
            // 2. Save context
            "stmdb   r0!, {{r4-r11}}",
            "mov     r1, #0", // no tz context supported yet. reserved.
            "mrs     r2, psplim",
            "mov     r3, lr",
            "mrs     r4, control",
            "stmdb   r0!, {{r1-r4}}",
            // 3. Update stack pointer
            "tst     lr, #0x04",
            "ite     eq",
            "msreq   msp, r0",
            "msrne   psp, r0", // If using PSP, update PSP
            // 4. Call C handler function
            "push    {{lr}}",    // save origin lr
            "bl      HardFault", // Call C handler function
            "pop     {{lr}}",
            "bx      lr",
        )
    }
}

#[cfg(not(armv6m))]
#[link_section = ".text.vector_handlers"]
#[linkage = "weak"]
#[no_mangle]
pub unsafe extern "C" fn MemManage_Handler() {
    HardFault_Handler();
}

#[cfg(not(armv6m))]
#[link_section = ".text.vector_handlers"]
#[linkage = "weak"]
#[no_mangle]
pub unsafe extern "C" fn BusFault_Handler() {
    HardFault_Handler();
}

#[link_section = ".text.vector_handlers"]
#[linkage = "weak"]
#[no_mangle]
pub unsafe extern "C" fn UsageFault_Handler() {
    HardFault_Handler();
}

#[cfg(armv8m)]
#[link_section = ".text.vector_handlers"]
#[linkage = "weak"]
#[no_mangle]
pub unsafe extern "C" fn SecureFault_Handler() {
    Default_Handler();
}

#[link_section = ".text.vector_handlers"]
#[linkage = "weak"]
#[no_mangle]
#[naked]
pub unsafe extern "C" fn SVCall_Handler() {
    unsafe {
        naked_asm!(
            "cpsid I", // FIXME: SVC might be interrupted by hardware interrupt.
            "mrs r0, psp",
            "push {{lr}}",
            "bl     SVCall",
            "pop {{lr}}",
            "cpsie I",
            "bx lr",
        )
    }
}

#[link_section = ".text.vector_handlers"]
#[linkage = "weak"]
#[no_mangle]
pub unsafe extern "C" fn DebugMon_Handler() {
    Default_Handler();
}

#[link_section = ".text.vector_handlers"]
#[linkage = "weak"]
#[no_mangle]
pub unsafe extern "C" fn SysTick_Handler() {
    Default_Handler();
}

#[doc(hidden)]
#[link_section = ".vector_table.exceptions"]
#[no_mangle]
pub static __EXCEPTIONS: [Vector; 15] = [
    // Exception 0: Reset.
    Vector {
        handler: Reset_Handler,
    },
    // Exception 2: Non Maskable Interrupt.
    Vector {
        handler: NMI_Handler,
    },
    // Exception 3: Hard Fault Interrupt.
    Vector {
        handler: HardFault_Handler,
    },
    // Exception 4: Memory Management Interrupt [not on Cortex-M0 variants].
    #[cfg(not(armv6m))]
    Vector {
        handler: MemManage_Handler,
    },
    #[cfg(armv6m)]
    Vector { reserved: 0 },
    // Exception 5: Bus Fault Interrupt [not on Cortex-M0 variants].
    #[cfg(not(armv6m))]
    Vector {
        handler: BusFault_Handler,
    },
    #[cfg(armv6m)]
    Vector { reserved: 0 },
    // Exception 6: Usage Fault Interrupt [not on Cortex-M0 variants].
    #[cfg(not(armv6m))]
    Vector {
        handler: UsageFault_Handler,
    },
    #[cfg(armv6m)]
    Vector { reserved: 0 },
    // Exception 7: Secure Fault Interrupt [only on Armv8-M].
    #[cfg(armv8m)]
    Vector {
        handler: SecureFault_Handler,
    },
    #[cfg(not(armv8m))]
    Vector { reserved: 0 },
    // 8-10: Reserved
    Vector { reserved: 0 },
    Vector { reserved: 0 },
    Vector { reserved: 0 },
    // Exception 11: SV Call Interrupt.
    Vector {
        handler: SVCall_Handler,
    },
    // Exception 12: Debug Monitor Interrupt [not on Cortex-M0 variants].
    #[cfg(not(armv6m))]
    Vector {
        handler: DebugMon_Handler,
    },
    #[cfg(armv6m)]
    Vector { reserved: 0 },
    // 13: Reserved
    Vector { reserved: 0 },
    // Exception 14: Pend SV Interrupt [not on Cortex-M0 variants].
    Vector {
        handler: PendSV_Handler,
    },
    // Exception 15: System Tick Interrupt.
    Vector {
        handler: SysTick_Handler,
    },
];
