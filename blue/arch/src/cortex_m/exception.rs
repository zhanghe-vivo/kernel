use crate::arch::stack_frame::{ExceptionFrame, StackFrame};
use crate::cortex_m::Arch;
use crate::interrupt::IInterrupt;
use core::{arch::asm, fmt};
use cortex_m::peripheral::SCB;

#[no_mangle]
#[naked]
pub unsafe extern "C" fn HardFault_Handler() {
    #[cfg(any(armv7m, armv7em))]
    unsafe {
        asm!(
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
            "orr     lr, lr, #0x04",
            "bx      lr",
            options(noreturn),
        )
    }

    #[cfg(armv8m)] // support trustzone
    unsafe {
        asm!(
            "mrs     r0, msp",   // get fault context from handler.
            "tst     lr, #0x04", // if(!EXC_RETURN[2])
            "beq     1f",
            "mrs     r0, psp",              // get fault context from thread.
            "1: stmfd   r0!, {{r4 - r11}}", // push r4 - r11 register.
            "ldr     r2,  =rt_trustzone_current_context",
            "ldr     r2, [r2]",           // r2 = *r2
            "mov     r3, lr",             //r3 = lr
            "mrs     r4, psplim",         //r4 = psplim
            "mrs     r5, control",        //r5 = control
            "stmfd   r0!, {{r2-r5, lr}}", //push to thread stack
            "tst     lr, #0x04",          //if(!EXC_RETURN[2])
            "beq     2f",
            "msr     psp, r0", //update stack pointer to PSP.
            "B       3f",
            "2: msr  msp, r0", //update stack pointer to MSP.
            "3: push {{lr}}",
            "bl      HardFault",
            "pop     {{lr}}",
            "orr     lr, lr, #0x04",
            "bx      lr",
            options(noreturn),
        )
    }
}

struct HardFaultRegs {
    cfsr: u32,
    hfsr: u32,
    mmfar: u32,
    bfar: u32,
}

impl HardFaultRegs {
    pub fn from_scb() -> Self {
        // 获取 SCB 寄存器的值
        let scb = unsafe { &*SCB::PTR };

        Self {
            cfsr: scb.cfsr.read(),
            hfsr: scb.hfsr.read(),
            mmfar: scb.mmfar.read(),
            bfar: scb.bfar.read(),
        }
    }
}

impl fmt::Display for HardFaultRegs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "\nHFSR: 0x{:08x}", self.hfsr)?;
        if self.hfsr & (1 << 30) != 0 {
            writeln!(f, "  - Forced Hard Fault")?;
        }
        if self.hfsr & (1 << 31) != 0 {
            writeln!(f, "  - Debug Event")?;
        }
        writeln!(f, "Fault Status:")?;
        if self.cfsr & 0xFF != 0 {
            writeln!(f, "  Memory Management Fault:")?;
            if self.cfsr & (1 << 0) != 0 {
                writeln!(f, "    - Instruction access violation")?;
            }
            if self.cfsr & (1 << 1) != 0 {
                writeln!(f, "    - Data access violation")?;
            }
            if self.cfsr & (1 << 3) != 0 {
                writeln!(f, "    - Unstacking error")?;
            }
            if self.cfsr & (1 << 4) != 0 {
                writeln!(f, "    - Stacking error")?;
            }
            writeln!(f, "    Fault Address: 0x{:08x}", self.mmfar)?;
        }

        if self.cfsr & 0xFF00 != 0 {
            writeln!(f, "  Bus Fault:")?;
            if self.cfsr & (1 << 8) != 0 {
                writeln!(f, "    - Instruction bus error")?;
            }
            if self.cfsr & (1 << 9) != 0 {
                writeln!(f, "    - Precise data bus error")?;
            }
            if self.cfsr & (1 << 10) != 0 {
                writeln!(f, "    - Imprecise data bus error")?;
            }
            writeln!(f, "    Fault Address: 0x{:08x}", self.bfar)?;
        }

        if self.cfsr & 0xFFFF0000 != 0 {
            writeln!(f, "  Usage Fault:")?;
            if self.cfsr & (1 << 16) != 0 {
                writeln!(f, "    - Undefined instruction")?;
            }
            if self.cfsr & (1 << 17) != 0 {
                writeln!(f, "    - Invalid state")?;
            }
            if self.cfsr & (1 << 18) != 0 {
                writeln!(f, "    - Invalid PC load")?;
            }
            if self.cfsr & (1 << 19) != 0 {
                writeln!(f, "    - Division by zero")?;
            }
        }

        Ok(())
    }
}

#[no_mangle]
pub unsafe extern "C" fn HardFault(ef: &ExceptionFrame) -> ! {
    Arch::disable_interrupts();
    let fault_regs: HardFaultRegs = HardFaultRegs::from_scb();

    panic!("\n=== HARD FAULT ===\n\n{}\n{} ", fault_regs, ef);

    // TODO: add print thread info
    loop {}
}
