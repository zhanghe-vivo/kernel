use crate::aarch64_cortex_a::Arch;
use core::arch::{asm, naked_asm};

/// Represents different DSB options for the AArch64 architecture
#[derive(Debug, Clone, Copy)]
pub enum DsbOptions {
    InnerShareable,
    OuterShareable,
    NonShareable,
    Sys,
}

impl Arch {
    #[inline]
    pub fn wait_for_interrupt() {
        // SAFETY: This doesn't access memory in any way.
        unsafe {
            asm!("wfi", options(nomem, nostack));
        }
    }

    #[inline]
    pub fn signal_event() {
        // SAFETY: This doesn't access memory in any way.
        unsafe {
            asm!("sev", options(nomem, nostack));
        }
    }

    #[inline]
    pub fn wait_for_event() {
        // SAFETY: This doesn't access memory in any way.
        unsafe {
            asm!("wfe", options(nomem, nostack));
        }
    }

    #[inline]
    pub fn sys_reset() -> ! {
        loop {
            // TBD
        }
    }

    #[inline]
    pub fn isb() {
        // SAFETY: This doesn't access memory in any way.
        unsafe {
            asm!("isb", options(nostack, nomem, preserves_flags));
        }
    }

    pub fn isb_sy() {
        // SAFETY: This doesn't access memory in any way.
        unsafe {
            asm!("isb sy", options(nostack, nomem, preserves_flags));
        }
    }

    /// Execute a DSB instruction with the given option
    pub fn dsb(option: DsbOptions) {
        // SAFETY: This doesn't access memory in any way.
        unsafe {
            match option {
                DsbOptions::InnerShareable => {
                    asm!("dsb ish", options(nostack, nomem, preserves_flags))
                }
                DsbOptions::OuterShareable => {
                    asm!("dsb osh", options(nostack, nomem, preserves_flags))
                }
                DsbOptions::NonShareable => {
                    asm!("dsb nsh", options(nostack, nomem, preserves_flags))
                }
                DsbOptions::Sys => asm!("dsb sy", options(nostack, nomem, preserves_flags)),
            }
        }
    }

    // clear tlb
    pub fn tlbi_all() {
        unsafe {
            asm!("tlbi vmalle1", options(nostack, preserves_flags));
        }
    }
}
