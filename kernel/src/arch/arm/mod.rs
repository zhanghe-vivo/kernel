// Copyright (c) 2025 vivo Mobile Communication Co., Ltd.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//       http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

pub(crate) mod hardfault;
pub(crate) mod irq;
pub(crate) mod xpsr;

pub(crate) use hardfault::handle_hardfault;

use crate::{
    scheduler,
    support::{sideeffect, Region, RegionalObjectBuilder},
    syscalls::{dispatch_syscall, Context as ScContext},
};
use core::{
    fmt,
    mem::offset_of,
    sync::{atomic, atomic::Ordering},
};
use cortex_m::peripheral::SCB;
use scheduler::ContextSwitchHookHolder;

pub const EXCEPTION_LR: usize = 0xFFFFFFFD;
pub const CONTROL: usize = 0x2;
pub const THUMB_MODE: usize = 0x01000000;
pub const NR_SWITCH: usize = !0;
pub const DISABLE_LOCAL_IRQ_BASEPRI: u8 = irq::IRQ_PRIORITY_FOR_SCHEDULER;

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
    current.saved_sp()
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
            "cpsie i",
            "bx r0",
            thumb = const THUMB_MODE,
            ctrl = const CONTROL,
            prepare = sym prepare_schedule,
            stack_end = sym __sys_stack_end,
        )
    }
}

#[repr(C, align(8))]
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

#[repr(C, align(8))]
#[derive(Default)]
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

impl fmt::Debug for IsrContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "IsrContext {{")?;
        write!(f, "r0: 0x{:x} ", self.r0)?;
        write!(f, "r1: 0x{:x} ", self.r1)?;
        write!(f, "r2: 0x{:x} ", self.r2)?;
        write!(f, "r3: 0x{:x} ", self.r3)?;
        write!(f, "r12: 0x{:x} ", self.r12)?;
        write!(f, "lr: 0x{:x} ", self.lr)?;
        write!(f, "pc: 0x{:x} ", self.pc)?;
        write!(f, "xpsr: 0x{:x} ", self.xpsr)?;
        write!(f, "}}")?;
        Ok(())
    }
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
            ldr r12, ={basepri}
            msr basepri, r12
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
            ldr r12, =0
            msr basepri, r12
            bx lr
            ",
        ),
        syscall_handler = sym handle_syscall,
        basepri = const DISABLE_LOCAL_IRQ_BASEPRI,
    )
}

extern "C" fn syscall_handler(ctx: &mut Context) {
    let sc = ScContext {
        nr: ctx.r7,
        args: [ctx.r0, ctx.r1, ctx.r2, ctx.r3, ctx.r4, ctx.r5],
    };
    // r0 should contain the return value.
    ctx.r0 = dispatch_syscall(&sc);
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
            tst lr, #0x200
            beq 1f
            pop {{lr}}
            1:
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
    let saved_sp_ptr: *mut usize = unsafe { ctx.r0 as *mut usize };
    if !saved_sp_ptr.is_null() {
        // FIXME: rustc opt the write out if not setting it volatile.
        unsafe {
            sideeffect();
            saved_sp_ptr.write_volatile(sp)
        };
    }
    let hook: *mut ContextSwitchHookHolder = unsafe { ctx.r2 as *mut ContextSwitchHookHolder<'_> };
    if !hook.is_null() {
        unsafe {
            sideeffect();
            scheduler::save_context_finish_hook(Some(&mut *hook));
        }
    }
    ctx.r1
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
    let dup_ctx = rb.write_after_start::<Context>(*ctx).unwrap() as *mut Context as *mut usize;
    // Check if re-alignment happened.
    // See https://developer.arm.com/documentation/ddi0403/d/System-Level-Architecture/System-Level-Programmers--Model/ARMv7-M-exception-model/Stack-alignment-on-exception-entry
    let realigned = (ctx.xpsr & (1 << 9)) != 0;
    unsafe {
        sideeffect();
        dup_ctx
            .byte_offset(offset_of!(Context, pc) as isize)
            .write_volatile(syscall_stub as usize);
        dup_ctx
            .byte_offset(offset_of!(Context, r0) as isize)
            .write_volatile(ctx as *const _ as usize);
        // The duplicated context doesn't need realignment.
        dup_ctx
            .byte_offset(offset_of!(Context, xpsr) as isize)
            .write_volatile(ctx.xpsr & !(1 << 9))
    }
    // We are playing a trick here, so that we can use `pop {{pc}}`
    // instruction, without using any scratch register.
    // Enforce thumb mode.
    let pc = ctx.pc | 1;
    let lr = ctx.lr;
    ctx.lr = ctx.xpsr;
    ctx.pc = lr;
    if realigned {
        unsafe {
            sideeffect();
            ctx.xpsr = lr;
            let reserved_ptr: *mut usize =
                (ctx as *mut _ as *mut usize).byte_offset(core::mem::size_of::<Context>() as isize);
            reserved_ptr.write_volatile(pc);
        }
    } else {
        ctx.xpsr = pc;
    }
    base
}

#[naked]
pub unsafe extern "C" fn handle_pendsv() {
    core::arch::naked_asm!(
        concat!(
            "
            ldr r12, ={basepri}
            msr basepri, r12
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
            ldr r12, =0
            msr basepri, r12
            bx lr
            "
        ),
        next_thread_sp = sym scheduler::yield_me_and_return_next_sp,
        basepri = const DISABLE_LOCAL_IRQ_BASEPRI,
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
        self
    }
}

#[inline]
pub extern "C" fn enable_local_irq() {
    unsafe {
        core::arch::asm!(
            "msr basepri, {}",
            in(reg) 0,
            options(nostack)
        )
    }
}

#[inline]
pub extern "C" fn disable_local_irq() {
    unsafe {
        core::arch::asm!(
            "msr basepri, {}",
            in(reg) DISABLE_LOCAL_IRQ_BASEPRI,
            options(nostack),
        )
    }
}

#[coverage(off)]
#[cfg_attr(debug, inline(never))]
pub extern "C" fn disable_local_irq_save() -> usize {
    let old: usize;
    unsafe {
        core::arch::asm!(
            concat!(
                "
                mrs {old}, basepri
                msr basepri, {val}
                ",
            ),
            old = out(reg) old,
            val = in(reg) DISABLE_LOCAL_IRQ_BASEPRI,
            options(nostack)
        )
    }
    atomic::compiler_fence(Ordering::SeqCst);
    old
}

#[coverage(off)]
#[cfg_attr(debug, inline(never))]
pub extern "C" fn enable_local_irq_restore(old: usize) {
    atomic::compiler_fence(Ordering::SeqCst);
    unsafe {
        core::arch::asm!(
        "msr basepri, {}", 
        in(reg) old,
        options(nostack))
    }
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
            "movs r12, r7",
            "ldr r7, ={nr}",
            "svc 0",
            "mov r7, r12",
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
            "mrs {}, basepri",
            out(reg) x, options(nostack)
        );
    };
    x == 0
}

#[inline]
pub extern "C" fn is_in_interrupt() -> bool {
    cortex_m::peripheral::SCB::vect_active() != cortex_m::peripheral::scb::VectActive::ThreadMode
}
