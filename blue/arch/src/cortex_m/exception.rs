use crate::{
    arch::{
        register::xpsr,
        stack_frame::{ExceptionFrame, ExceptionFrameFpu, StackSettings},
    },
    cortex_m::Arch,
    interrupt::IInterrupt,
};
use core::{arch::naked_asm, fmt};
use cortex_m::peripheral::SCB;

#[cfg(any(armv7m, armv7em,))]
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
#[no_mangle]
#[naked]
pub unsafe extern "C" fn HardFault_Handler() {
    unsafe {
        naked_asm!(
            // 1. 确定使用的栈指针
            "tst     lr, #0x04", // 检查 SPSEL 位
            "ite     eq",        // if-then-else 块
            "mrseq   r0, msp",   // 如果 SPSEL 位为 0，使用 MSP
            "mrsne   r0, psp",   // 如果 SPSEL 位为 1，使用 PSP
            // 2. 保存上下文
            "stmdb   r0!, {{r4-r11}}", // 保存寄存器
            "mov     r1, #0",          // no tz context supported yet. reserved.
            "mrs     r2, psplim",      // 获取 PSPLIM
            "mov     r3, lr",          // EXC_RETURN 值
            "mrs     r4, control",     // CONTROL 寄存器
            "stmdb   r0!, {{r1-r4}}",  // 保存寄存器
            // 3. 更新栈指针
            "tst     lr, #0x04", // 再次检查栈指针
            "ite     eq",
            "msreq   msp, r0",
            "msrne   psp, r0", // 如果使用 PSP，更新 PSP
            // 4. 调用 C 处理函数
            "push    {{lr}}",    // save origin lr
            "bl      HardFault", // 调用 C 处理函数
            "pop     {{lr}}",
            "bx      lr",
        )
    }
}

struct HardFaultRegs {
    cfsr: u32,  // Configurable Fault Status Register
    hfsr: u32,  // Hard Fault Status Register
    mmfar: u32, // Memory Management Fault Address Register
    bfar: u32,  // Bus Fault Address Register
    afsr: u32,  // Auxiliary Fault Status Register (ARMv8-M)
}

impl HardFaultRegs {
    pub fn from_scb() -> Self {
        // Get the value of the SCB registers
        // SAFETY: SCB::PTR comes from cortex_m crate and is a valid pointer
        let scb = unsafe { &*SCB::PTR };

        Self {
            cfsr: scb.cfsr.read(),
            hfsr: scb.hfsr.read(),
            mmfar: scb.mmfar.read(),
            bfar: scb.bfar.read(),
            afsr: scb.afsr.read(),
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
        if self.hfsr & (1 << 1) != 0 {
            writeln!(f, "  - Vector Table Read Fault")?;
        }
        writeln!(f, "CFSR: 0x{:08x}", self.cfsr)?;
        writeln!(f, "Fault Status:")?;
        if self.cfsr & 0xFF != 0 {
            // https://developer.arm.com/documentation/ddi0553/latest/
            // MMFARVALID, bit [7] - MMFAR valid flag. Indicates validity of the MMFAR register.
            //                       0: MMFAR content not valid.
            //                       1: MMFAR content valid
            // MLSPERR, bit [5]    - MemManage lazy Floating-point state preservation error flag.
            //                       Records whether a MemManage fault occurred during lazy Floating-point state preservation.
            //                       0: No MemManage occurred.
            //                       1: MemManage occurred.
            // MSTKERR, bit [4]    - MemManage stacking error flag. Records whether a derived MemManage fault occurred during exception entry stacking.
            //                       0: No derived MemManage occurred.
            //                       1: Derived MemManage occurred during exception entry.
            // MUNSTKERR, bit [3]  - MemManage unstacking error flag. Records whether a derived MemManage fault occurred during exception return unstacking.
            //                       0: No derived MemManage fault occurred.
            //                       1: Derived MemManage fault occurred during exception return
            // DACCVIOL, bit [1]   - Data access violation flag. Records whether a data access violation has occurred.
            //                       0: No MemManage fault on data access has occurred.
            //                       1:  MemManage fault on data access has occurred.
            //                       A DACCVIOL will be accompanied by an MMFAR update.
            // IACCVIOL, bit [0]   - Instruction access violation. Records whether an instruction related memory access violation has occurred.
            //                       0: No MemManage fault on instruction access has occurred.
            //                       1: MemManage fault on instruction access has occurred.
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
            if self.cfsr & (1 << 5) != 0 {
                writeln!(f, "    - lazy Floating-point state preservation error")?;
            }
            if self.cfsr & (1 << 7) != 0 {
                writeln!(f, "    - MMFAR valid")?;
                writeln!(f, "      Fault Address: 0x{:08x}", self.mmfar)?;
            }
        }
        if self.cfsr & 0xFF00 != 0 {
            // BFARVALID, bit [7] - BFAR valid. Indicates validity of the contents of the BFAR register.
            //                      0: BFAR content not valid.
            //                      1: BFAR content valid.
            // LSPERR, bit [5]   - Lazy state preservation error. Records whether a precise BusFault occurred during floating-point lazy
            //                     Floating-point state preservation.
            //                      0: No BusFault occurred.
            //                      1: BusFault occurred.
            //                     If AIRCR.BFHFNMINS is zero this bit is RAZ/WI from Non-secure state.
            // STKERR, bit [4]   - Stack error. Records whether a precise derived BusFault occurred during exception entry stacking.
            //                      0: No derived BusFault occurred.
            //                      1: Derived BusFault occurred during exception entry.
            //                    Derived BusFault occurred during exception entry.
            //                    If AIRCR.BFHFNMINS is zero this bit is RAZ/WI from Non-secure state.
            // UNSTKERR, bit [3] - Unstack error. Records whether a precise derived BusFault occurred during exception return unstacking.
            //                      0 :No derived BusFault occurred.
            //                      1: Derived BusFault occurred during exception return.
            //                     If AIRCR.BFHFNMINS is zero this bit is RAZ/WI from Non-secure state.
            // IMPRECISERR, bit [2] - Imprecise error. Records whether an imprecise data access error has occurred.
            //                      0: No imprecise data access error has occurred.
            //                      1: Imprecise data access error has occurred.
            //                     If AIRCR.BFHFNMINS is zero this bit is RAZ/WI from Non-secure state.
            // PRECISERR, bit [1] - Precise error. Records whether a precise data access error has occurred.
            //                      0: No precise data access error has occurred.
            //                      1: Precise data access error has occurred.
            //                     When a precise error is recorded, the associated address is written to the BFAR and BFSR.BFARVALID bit
            //                     is set.
            //                     If AIRCR.BFHFNMINS is zero this bit is RAZ/WI from Non-secure state.
            // IBUSERR, bit [0] - Instruction bus error. Records whether a precise BusFault on an instruction prefetch has occurred.
            //                      0: No BusFault on instruction prefetch has occurred.
            //                      1: A BusFault on an instruction prefetch has occurred.
            //                     An IBUSERR is only recorded if the instruction is issued for execution.
            //                     If AIRCR.BFHFNMINS is zero this bit is RAZ/WI from Non-secure state.
            writeln!(f, "  Bus Fault:")?;
            if self.cfsr & (1 << 8) != 0 {
                writeln!(f, "    - Instruction bus error")?;
            }
            if self.cfsr & (1 << 9) != 0 {
                writeln!(f, "    - Precise error")?;
            }
            if self.cfsr & (1 << 10) != 0 {
                writeln!(f, "    - Imprecise error")?;
            }
            if self.cfsr & (1 << 11) != 0 {
                writeln!(f, "    - Unstack error")?;
            }
            if self.cfsr & (1 << 12) != 0 {
                writeln!(f, "    - Stacking error")?;
            }
            if self.cfsr & (1 << 13) != 0 {
                writeln!(f, "    - Lazy state preservation error")?;
            }
            if self.cfsr & (1 << 15) != 0 {
                writeln!(f, "    - BFAR valid")?;
                writeln!(f, "      Fault Address: 0x{:08x}", self.bfar)?;
            }
        }
        // DIVBYZERO, bit [9] - Divide by zero flag. Sticky flag indicating whether an integer division by zero error has occurred.
        //                      0: Error has not occurred.
        //                      1: Error has occurred.
        // UNALIGNED, bit [8] - Unaligned access flag. Sticky flag indicating whether an unaligned access error has occurred.
        //                      0: Error has not occurred.
        //                      1: Error has occurred.
        // Bits [7:5]         -  Reserved, RES0.
        // STKOF, bit [4]     - Stack overflow flag. Sticky flag indicating whether a stack overflow error has occurred.
        //                      0: Error has not occurred.
        //                      1: Error has occurred.
        // NOCP, bit [3]       - No coprocessor flag. Sticky flag indicating whether a coprocessor disabled or not present error has occurred.
        //                      0: Error has not occurred.
        //                      1: Error has occurred.
        // INVPC, bit [2]      - Invalid PC flag. Sticky flag indicating whether an integrity check error has occurred.
        //                      0: Error has not occurred.
        //                      1: Error has occurred.
        // INVSTATE, bit [1]    - Invalid state flag. Sticky flag indicating whether an EPSR.B, EPSR.T, EPSR.IT, or FPSCR.LTPSIZE validity
        //                      0: Error has not occurred.
        //                      1: Error has occurred.
        // UNDEFINSTR, bit [0]   - UNDEFINED instruction flag. Sticky flag indicating whether an UNDEFINED instruction error has occurred.
        //                      0: Error has not occurred.
        //                      1: Error has occurred.
        //                      This includes attempting to execute an UNDEFINED instruction associated with an enable coprocessor.
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
                writeln!(f, "    - No coprocessor")?;
            }
            #[cfg(armv8m)]
            if self.cfsr & (1 << 20) != 0 {
                writeln!(f, "    - Stack overflow")?;
            }
            if self.cfsr & (1 << 24) != 0 {
                writeln!(f, "    - Unaligned access")?;
            }
            if self.cfsr & (1 << 25) != 0 {
                writeln!(f, "    - Divide by zero")?;
            }
        }

        writeln!(f, "AFSR: 0x{:08x}", self.afsr)?;
        if self.afsr != 0 {
            writeln!(f, "  - Auxiliary Faults detected")?;
        }

        Ok(())
    }
}

#[no_mangle]
pub unsafe extern "C" fn HardFault(ef: &ExceptionFrame) -> ! {
    Arch::disable_interrupts();
    let fault_regs: HardFaultRegs = HardFaultRegs::from_scb();
    let xpsr = xpsr::read();
    let stack_xpsr = xpsr::Xpsr::from_bits(ef.base_frame.xpsr);

    panic!(
        "\n=== HARD FAULT ===\n{}\n{}\n{}\n stack xpsr: {} ",
        fault_regs, xpsr, ef, stack_xpsr
    );
}
