use crate::arch::{registers::cntpct_el0::CNTPCT_EL0, stack_frame::StackFrame, Arch};
use core::{
    arch::{asm, naked_asm},
    mem,
};
use tock_registers::interfaces::Readable;

impl Arch {
    /// This function will initialize thread stack
    /// tentry the entry of thread
    /// parameter the parameter of entry
    /// stack_addr the beginning stack address
    /// texit the function will be called when thread exit
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
        stack_frame.x0 = arg as u64;
        stack_frame.elr = entry as u64;
        stack_frame.lr = exit as u64;
        // EL1 with SP_EL1 (EL1h)
        stack_frame.spsr = 0b0101;
        stack_ptr.offset(-(stack_offset as isize))
    }

    pub fn context_switch_to(stack_ptr: *const usize) -> ! {
        unsafe {
            asm!(
                concat!(
                    "ldr x0, [x0]\n",
                    "mov sp, x0\n",
                    crate::restore_context!(),
                    "eret\n",
                ),
                in("x0") stack_ptr as u64,
                options(noreturn),
            )
        }
    }

    #[naked]
    pub fn context_switch(from: *const usize, to: *const usize) {
        // SAFETY: Safe bare metal assembly operations
        unsafe {
            naked_asm!(concat!(
                crate::save_general_purpose_reg!(),
                "mov x3, #((3 << 6) | 0x05)\n",
                "mov x2, x30\n",
                "stp x2, x3, [sp, #-16]!\n",
                "mov x9, sp\n",
                "str x9, [x0]\n",
                "str x1, [sp, #-0x8]!\n",
                "bl unlock_in_ctx_switch\n",
                "ldr x1, [sp], #0x8\n",
                "ldr x10, [x1]\n",
                "mov sp, x10\n",
                crate::restore_context!(),
                "eret\n",
            ),)
        }
    }

    pub fn get_cycle_count() -> u64 {
        CNTPCT_EL0.get()
    }
}
