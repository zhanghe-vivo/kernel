use crate::arch::{registers::esr_el1::ESR_EL1, stack_frame::StackFrame, Arch};
use core::arch::{asm, naked_asm};
use tock_registers::interfaces::Readable;

#[link_section = ".text._exception"]
#[no_mangle]
#[naked]
unsafe extern "C" fn el1_fiq() {
    naked_asm!(concat!(
        "msr DAIFSet, #0x3\n",
        crate::save_context!(),
        "bl trap_fiq\n",
        crate::restore_context!(),
        "eret"
    ));
}

#[link_section = ".text._exception"]
#[no_mangle]
unsafe extern "C" fn el0_not_supported() {
    panic!("el0 is not supported.");
    asm!("b .");
}

#[link_section = ".text._exception"]
#[no_mangle]
unsafe extern "C" fn lowerel_not_supported() {
    panic!("lowerel is not supported.");
    asm!("b .");
}

#[link_section = ".text._exception"]
#[no_mangle]
#[naked]
unsafe extern "C" fn el1_sync() {
    naked_asm!(concat!(
        "msr DAIFSet, #0x3\n",
        crate::save_context!(),
        "mov x0, sp\n",
        "bl trap_exception\n",
        crate::restore_context!(),
        "eret"
    ));
}

#[link_section = ".text._exception"]
#[no_mangle]
#[naked]
unsafe extern "C" fn el1_irq() {
    naked_asm!(concat!(
        "msr DAIFSet, #0x3\n",
        crate::save_context!(),
        "bl trap_irq\n",
        "mov x0, sp\n",
        "bl switch_context_in_irq\n",
        "mov sp, x0\n",
        crate::restore_context!(),
        "eret"
    ));
}

#[link_section = ".text._exception"]
#[no_mangle]
#[naked]
unsafe extern "C" fn el1_error() {
    naked_asm!(concat!(
        "msr DAIFSet, #0x3\n",
        crate::save_context!(),
        "mov x0, sp\n",
        "bl trap_exception\n",
        "b .\n",
    ));
}

#[link_section = ".text._exception"]
#[no_mangle]
extern "C" fn trap_exception(stack_frame: &mut StackFrame) {
    let esr = ESR_EL1.get();
    let ec = (esr >> 26) & 0x3F;
    if ec == 0x15 {
        extern "C" {
            fn svcall0(_: u64, _: u64, _: u64, _: u64, _: u64, _: u64, _: u64) -> u64;
        }
        unsafe {
            stack_frame.x0 = svcall0(
                stack_frame.x0,
                stack_frame.x1,
                stack_frame.x2,
                stack_frame.x3,
                stack_frame.x4,
                stack_frame.x5,
                stack_frame.x8,
            );
        }
    } else {
        show_exception(ec, stack_frame);
    }
}

#[link_section = ".text._exception"]
#[no_mangle]
extern "C" fn trap_irq() {
    let irq = Arch::get_interrupt();
    Arch::trigger_irq(irq);
    Arch::end_interrupt(irq);
}

#[link_section = ".text._exception"]
#[no_mangle]
extern "C" fn trap_fiq() {
    let fiq = Arch::get_interrupt();
    if u32::from(fiq) != 1023 {
        Arch::trigger_irq(fiq);
    }
    Arch::end_interrupt(fiq);
}

fn show_exception(ec: u64, stack_frame: &mut StackFrame) {
    match ec {
        0x00 => panic!("Unknow reason Exceptions\n======== error stack ======== \n{}",stack_frame),
        0x01 => panic!("WFI or WFE instruction\n======== error stack ======== \n{}",stack_frame),
        0x03 => panic!("MCR or MRC access to CP15a that is not reported using EC 0x00\n======== error stack ======== \n{}",stack_frame),
        0x04 => panic!("MCRR or MRRC access to CP15a that is not reported using EC 0x00\n======== error stack ======== \n{}",stack_frame),
        0x05 => panic!("MCR or MRC access to CP14a\n======== error stack ======== \n{}",stack_frame),
        0x06 => panic!("LDC or STC access to CP14a\n======== error stack ======== \n{}",stack_frame),
        0x07 => panic!("Access to SIMD or floating-point registersa, excluding (HCR_EL2.TGE==1) traps\n======== error stack ======== \n{}",stack_frame),
        0x08 => panic!("MCR or MRC access to CP10 that is not reported using EC 0x07. This applies only to ID Group trapsd\n======== error stack ======== \n{}",stack_frame),
        0x0c => panic!("MRRC access to CP14a\n======== error stack ======== \n{}",stack_frame),
        0x0e => panic!("Illegal Execution State\n======== error stack ======== \n{}",stack_frame),
        0x11 => panic!("SVC call from Aarch32\n======== error stack ======== \n{}",stack_frame),
        0x12 => panic!("HVC instruction execution, when HVC is not disabled\n======== error stack ======== \n{}",stack_frame),
        0x13 => panic!("SMC instruction execution, when SMC is not disabled\n======== error stack ======== \n{}",stack_frame),
        0x15 => panic!("SVC call from AArch64 state\n======== error stack ======== \n{}",stack_frame),
        0x16 => panic!("HVC instruction execution, when HVC is not disabled\n======== error stack ======== \n{}",stack_frame),
        0x17 => panic!("SMC instruction execution, when SMC is not disabled\n======== error stack ======== \n{}",stack_frame),
        0x18 => panic!("MSR, MRS, or System instruction execution, that is not reported using EC 0x00, 0x01, or 0x07\n======== error stack ======== \n{}",stack_frame),
        0x20 => panic!("Instruction Abort from a lower Exception level\n======== error stack ======== \n{}",stack_frame),
        0x21 => panic!("Instruction Abort taken without a change in Exception level\n======== error stack ======== \n{}",stack_frame),
        0x22 => panic!("Misaligned PC exception\n======== error stack ======== \n{}",stack_frame),
        0x24 => panic!(" Data Abort from a lower Exception levelh\n======== error stack ======== \n{}",stack_frame),
        0x25 => panic!("Data Abort taken without a change in Exception level\n======== error stack ======== \n{}",stack_frame),
        0x26 => panic!("Stack Pointer Alignment exception\n======== error stack ======== \n{}",stack_frame),
        0x28 => panic!("Floating-point exception, if supported\n======== error stack ======== \n{}",stack_frame),
        0x2C => panic!("Floating-point exception, if supported\n======== error stack ======== \n{}",stack_frame),
        0x2F => panic!("SError interrupt\n======== error stack ======== \n{}",stack_frame),
        0x30 => panic!("Breakpoint exception from a lower Exception level\n======== error stack ======== \n{}",stack_frame),
        0x31 => panic!("Breakpoint exception taken without a change in Exception level\n======== error stack ======== \n{}",stack_frame),
        0x32 => panic!("Software Step exception from a lower Exception level\n======== error stack ======== \n{}",stack_frame),
        0x33 => panic!("Software Step exception taken without a change in Exception level\n======== error stack ======== \n{}",stack_frame),
        0x34 => panic!("Watchpoint exception from a lower Exception level\n======== error stack ======== \n{}",stack_frame),
        0x35 => panic!("Watchpoint exception taken without a change in Exception level\n======== error stack ======== \n{}",stack_frame),
        0x38 => panic!("BKPT instruction execution\n======== error stack ======== \n{}",stack_frame),
        0x3A => panic!("Vector catch exception from AArch32 state\n======== error stack ======== \n{}",stack_frame),
        0x3C => panic!("BRK instruction execution\n======== error stack ======== \n{}",stack_frame),
        _ => todo!(),
    }
}
