use crate::aarch64_cortex_a::{interrupt, mmu, vector};

#[no_mangle]
#[naked]
pub unsafe extern "C" fn _start() -> ! {
    // start at el2
    crate::cpu_in_el2!(boot);
}

extern "C" {
    pub static mut __bss_start: u8;
    pub static mut __bss_end: u8;
    pub static mut __sys_stack_start: u8;
    pub static mut __sys_stack_end: u8;
}

#[no_mangle]
#[naked]
pub unsafe extern "C" fn boot() -> ! {
    crate::arch_bootstrap!(__sys_stack_start, __sys_stack_end, init);
}

#[macro_export]
macro_rules! arch_bootstrap {
    ($stack_start:path, $stack_end:path, $cont:path) => {
        core::arch::naked_asm!(
            "
                ldr x1, = {stack_end}
                mov sp, x1
                b {cont}
            ",
            stack_end = sym $stack_end,
            cont = sym $cont,
        )
    };
}

#[no_mangle]
unsafe extern "C" fn init() -> ! {
    extern "C" {
        fn _startup() -> !;
    }
    init_bss();
    mmu::enable_mmu();
    interrupt::init();
    vector::vector_init();
    _startup();
}

#[inline]
unsafe fn init_bss() {
    let mut ptr = &raw mut __bss_start as *mut u8;
    while ptr != &raw mut __bss_end as *mut u8 {
        ptr.write(0u8);
        ptr = ptr.offset(1);
    }
}

#[macro_export]
macro_rules! cpu_in_el2 {
    ($cont:path) => {
        core::arch::naked_asm!(
            "
                // check cpu
                mrs x0, mpidr_el1
                and x0, x0, #0b11
                cbnz x0, 1f

                // Don't trap SIMD/FP instructions in both EL0 and EL1
                mov     x1, #0x00300000
                msr     cpacr_el1, x1

                // Enable CNTP to EL1 for systick
                mrs     x0, cnthctl_el2
                orr     x0, x0, #3
                msr     cnthctl_el2, x0
                msr     cntvoff_el2, xzr

                // Enable AArch64 in EL1
                mov x0, #(1 << 31)  
                orr x0, x0, #(1 << 1)
                msr hcr_el2, x0

                // Set EL1 sp and mask daif in EL2
                mov x0, #0x3C5
                msr spsr_el2, x0

                // set EL1 entry.
                adr x0, {cont}
                msr elr_el2, x0 

                // return el1
                eret
                // wait for event
                1:
                    wfe
                    b 1b
            ",
            cont = sym $cont,
        )
    };
}
