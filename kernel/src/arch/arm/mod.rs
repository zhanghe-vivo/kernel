pub(crate) mod hardfault;
pub(crate) mod xpsr;

use crate::{
    scheduler,
    support::{sideeffect, Region, RegionalObjectBuilder},
    syscalls::{dispatch_syscall, Context as ScContext},
};
use core::{
    fmt,
    mem::offset_of,
    ptr::addr_of,
    sync::{
        atomic,
        atomic::{compiler_fence, fence, Ordering},
    },
};
use cortex_m::peripheral::SCB;
use scheduler::ContextSwitchHookHolder;

pub const EXCEPTION_LR: usize = 0xFFFFFFFD;
pub const CONTROL: usize = 0x2;
pub const THUMB_MODE: usize = 0x01000000;
pub const NR_SWITCH: usize = !0;

#[macro_export]
macro_rules! arch_bootstrap {
    ($stack_start:expr, $stack_end:expr, $cont:path) => {
        core::arch::naked_asm!(
            "cpsid i",
            "b {cont}",
            cont = sym $cont,
        )
    };
}

extern "C" fn prepare_schedule(cont: extern "C" fn() -> !) -> usize {
    let current = scheduler::current_thread();
    current.lock().reset_saved_sp();
    return current.saved_sp();
}

extern "C" {
    pub static __sys_stack_start: u8;
    pub static __sys_stack_end: u8;
}

macro_rules! disable_interrupt {
    () => {
        "
        cpsid i
        "
    };
}

macro_rules! enable_interrupt {
    () => {
        "
        cpsie i
        "
    };
}

#[naked]
pub(crate) extern "C" fn start_schedule(cont: extern "C" fn() -> !) {
    unsafe {
        core::arch::naked_asm!(
            "mov r4, r0",
            "bl {prepare}",
            "msr psp, r0",
            "mov r0, r4",
            "ldr r12, ={stack_end}", // Reset MSP.
            "msr msp, r12",
            // Reset handler is special, see
            // https://stackoverflow.com/questions/59008284/if-the-main-function-is-called-inside-the-reset-handler-how-other-interrupts-ar
            "ldr r12, ={thumb}",
            "msr xpsr, r12",
            "ldr r12, ={ctrl}",
            "msr control, r12",
            "ldr lr, =0",
            "isb",
            "bx r0",
            thumb = const THUMB_MODE,
            ctrl = const CONTROL,
            prepare = sym prepare_schedule,
            stack_end = sym __sys_stack_end,
        )
    }
}

#[repr(C, align(4))]
#[derive(Default, Debug, Copy, Clone)]
pub struct Context {
    pub r4: usize,
    pub r5: usize,
    pub r6: usize,
    pub r7: usize,
    pub r8: usize,
    pub r9: usize,
    pub r10: usize,
    pub r11: usize,
    // Cortex-m saves R0, R1, R2, R3, R12, LR, PC, xPSR automatically
    // on psp, so they don't appear in the Context. Additionally, sp
    // == R13, lr == R14, pc == R15.
    pub r0: usize,
    pub r1: usize,
    pub r2: usize,
    pub r3: usize,
    pub r12: usize,
    pub lr: usize,
    pub pc: usize,
    pub xpsr: usize,
}

#[repr(C, align(4))]
#[derive(Default, Debug)]
pub struct IsrContext {
    pub r0: usize,
    pub r1: usize,
    pub r2: usize,
    pub r3: usize,
    pub r12: usize,
    pub lr: usize,
    pub pc: usize,
    pub xpsr: usize,
}

// FIXME: We need to pass a scratch register to perform saving.
// Use r12 as scratch register now.
macro_rules! store_callee_saved_regs {
    () => {
        "
        mrs r12, psp
        stmdb r12!, {{r4-r11}}
        "
    };
}

macro_rules! load_callee_saved_regs {
    () => {
        "
        ldmia r12!, {{r4-r11}}
        msr psp, r12
        "
    };
}

pub(crate) extern "C" fn post_pendsv() {
    SCB::set_pendsv();
    unsafe { core::arch::asm!("dsb", "isb", options(nostack),) }
}

#[naked]
pub(crate) unsafe extern "C" fn handle_svc() {
    core::arch::naked_asm!(
        concat!(
            "
            mrs r3, PRIMASK
            cpsid i
            ",
            store_callee_saved_regs!(),
            "
            mov r0, r12
            push {{r3, lr}}
            bl {syscall_handler}
            pop {{r3, lr}}
            mov r12, r0
            ",
            load_callee_saved_regs!(),
            "
            msr PRIMASK, r3
            bx lr
            ",
        ),
        syscall_handler = sym handle_syscall,
    )
}

extern "C" fn syscall_handler(ctx: &mut Context) {
    let mut sc = ScContext::default();
    sc.nr = ctx.r7;
    sc.args[0] = ctx.r0;
    sc.args[1] = ctx.r1;
    sc.args[2] = ctx.r2;
    sc.args[3] = ctx.r3;
    sc.args[4] = ctx.r4;
    sc.args[5] = ctx.r5;
    // r0 should contain the return value.
    ctx.r0 = dispatch_syscall(&sc) as usize;
}

#[naked]
unsafe extern "C" fn syscall_stub(ctx: *mut Context) {
    core::arch::naked_asm!(
        concat!(
            "
            push {{r0, lr}}
            bl {syscall_handler}
            pop {{r0, lr}}
            mov r12, r0
            ",
            load_callee_saved_regs!(),
            "
            pop {{r0-r3, r12}}
            pop {{lr}}
            pop {{lr}}
            pop {{pc}}
            ",
        ),
        syscall_handler = sym syscall_handler,
    )
}

#[inline(never)]
fn handle_svc_switch(ctx: &Context) -> usize {
    // r0 contains pointer to the saved_sp of the `from` thread, null
    // if saving context is not needed;
    // r1 contains the saved_sp of the `to` thread;
    // r2 contains the pointer to the switch hook holder, null if
    // there is no switch hook holder.
    assert_eq!(ctx.r7, NR_SWITCH);
    let sp = ctx as *const _ as usize;
    let saved_sp_ptr: *mut usize = unsafe { core::mem::transmute(ctx.r0) };
    if !saved_sp_ptr.is_null() {
        // FIXME: rustc opt the write out if not setting it volatile.
        unsafe {
            sideeffect();
            saved_sp_ptr.write_volatile(sp)
        };
    }
    let hook: *mut ContextSwitchHookHolder = unsafe { core::mem::transmute(ctx.r2) };
    if !hook.is_null() {
        unsafe {
            sideeffect();
            scheduler::save_context_finish_hook(Some(&mut *hook));
        }
    }
    return ctx.r1;
}

extern "C" fn handle_syscall(ctx: &mut Context) -> usize {
    if ctx.r7 == NR_SWITCH {
        return handle_svc_switch(ctx);
    }
    // Duplicate ctx so that we can exit to thread mode to
    // handle syscalls.
    let mut base = ctx as *const _ as usize;
    let size = core::mem::size_of::<Context>();
    base -= size;
    let region = Region { base, size };
    let mut rb = RegionalObjectBuilder::new(region);
    let dup_ctx =
        rb.write_after_start::<Context>(ctx.clone()).unwrap() as *mut Context as *mut usize;
    // Use thumb mode.
    ctx.xpsr = ctx.pc | 1;
    ctx.pc = ctx.lr;
    unsafe {
        dup_ctx
            .byte_offset(offset_of!(Context, pc) as isize)
            .write_volatile(syscall_stub as usize);
        dup_ctx
            .byte_offset(offset_of!(Context, r0) as isize)
            .write_volatile(ctx as *const _ as usize);
    }
    return base;
}

#[naked]
pub unsafe extern "C" fn handle_pendsv() {
    core::arch::naked_asm!(
        concat!(
            "
            mrs r3, PRIMASK
            cpsid i
            ",
            store_callee_saved_regs!(),
            "
            push {{r3, lr}}
            mov r0, r12
            bl {next_thread_sp}
            mov r12, r0
            pop {{r3, lr}}
            ",
            load_callee_saved_regs!(),
            "
            msr PRIMASK, r3
            bx lr
            "
        ),
        next_thread_sp = sym scheduler::yield_me_and_return_next_sp,
    )
}

impl Context {
    #[inline(never)]
    pub fn set_return_address(&mut self, pc: usize) -> &mut Self {
        self.pc = pc;
        self
    }

    #[inline]
    pub fn get_return_address(&self) -> usize {
        self.pc
    }

    #[inline]
    pub fn set_arg(&mut self, i: usize, val: usize) -> &mut Self {
        match i {
            0 => self.r0 = val,
            1 => self.r1 = val,
            2 => self.r2 = val,
            3 => self.r3 = val,
            _ => panic!("Should be passed by stack"),
        }
        self
    }

    #[inline]
    pub fn init(&mut self) -> &mut Self {
        self.xpsr = THUMB_MODE;
        return self;
    }
}

#[inline]
pub extern "C" fn enable_local_irq() {
    unsafe { core::arch::asm!(enable_interrupt!(), options(nostack)) }
}

#[inline]
pub extern "C" fn disable_local_irq() {
    unsafe { core::arch::asm!(disable_interrupt!(), options(nostack)) }
}

#[inline]
pub extern "C" fn disable_local_irq_save() -> usize {
    let old: usize;
    unsafe {
        core::arch::asm!(
            concat!(
                "mrs {}, PRIMASK",
                disable_interrupt!(),
            ),
            out(reg) old, options(nostack)
        )
    }
    atomic::compiler_fence(Ordering::SeqCst);
    old
}

#[inline]
pub extern "C" fn enable_local_irq_restore(old: usize) {
    atomic::compiler_fence(Ordering::SeqCst);
    unsafe { core::arch::asm!("msr PRIMASK, {}", in(reg) old, options(nostack)) }
}

#[inline]
pub extern "C" fn idle() {
    unsafe { core::arch::asm!("wfi") }
}

#[inline]
pub extern "C" fn current_sp() -> usize {
    let x: usize;
    unsafe { core::arch::asm!("mov {}, sp", out(reg) x, options(nostack, nomem)) };
    x
}

#[inline]
pub extern "C" fn current_msp() -> usize {
    let x: usize;
    unsafe { core::arch::asm!("mrs {}, msp", out(reg) x, options(nostack, nomem)) };
    x
}

#[inline]
pub extern "C" fn current_psp() -> usize {
    let x: usize;
    unsafe { core::arch::asm!("mrs {}, psp", out(reg) x, options(nostack, nomem)) };
    x
}

#[naked]
pub extern "C" fn switch_context_with_hook(
    saved_sp_mut: *mut u8,
    to_sp: usize,
    hook: *mut ContextSwitchHookHolder,
) {
    unsafe {
        core::arch::naked_asm!(
            "ldr r7, ={nr}",
            "svc 0",
            "bx lr",
            nr = const NR_SWITCH,
        )
    }
}

#[inline(always)]
pub extern "C" fn pend_switch_context() {
    post_pendsv();
}

#[inline(always)]
pub extern "C" fn switch_context(saved_sp_mut: *mut u8, to_sp: usize) {
    switch_context_with_hook(saved_sp_mut, to_sp, core::ptr::null_mut());
}

#[naked]
pub extern "C" fn save_context_in_isr(sp: &mut usize) {
    unsafe {
        core::arch::naked_asm!(
            concat!(
                store_callee_saved_regs!(),
                "
                mrs r1, psp
                str r1, [r0]
                bx lr
                ",
            ),
            options(),
        )
    }
}

#[inline(always)]
pub extern "C" fn restore_context(to_sp: usize) -> ! {
    switch_context_with_hook(core::ptr::null_mut(), to_sp, core::ptr::null_mut());
    unreachable!("Should have switched to another thread");
}

#[inline(always)]
pub extern "C" fn restore_context_with_hook(to_sp: usize, hook: *mut ContextSwitchHookHolder) -> ! {
    switch_context_with_hook(core::ptr::null_mut(), to_sp, hook);
    unreachable!("Should have switched to another thread");
}

#[naked]
pub extern "C" fn restore_context_in_isr(to_sp: usize) {
    unsafe {
        core::arch::naked_asm!(
            concat!(
                "
                mov r12, r0
                ",
                load_callee_saved_regs!(),
                "
                isb
                bx lr
                ",
            ),
            options(),
        )
    }
}

#[inline]
pub extern "C" fn current_cpu_id() -> usize {
    0
}

#[inline]
pub extern "C" fn local_irq_enabled() -> bool {
    let x: usize;
    unsafe {
        core::arch::asm!(
            "mrs {}, PRIMASK",
            out(reg) x, options(nostack)
        );
    };
    x == 0
}

#[inline]
pub extern "C" fn is_in_interrupt() -> bool {
    cortex_m::peripheral::SCB::vect_active() != cortex_m::peripheral::scb::VectActive::ThreadMode
}
