use tock_registers::{interfaces::Writeable, register_bitfields};

// See: https://developer.arm.com/documentation/ddi0595/2020-12/AArch64-Registers/SPSel--Stack-Pointer-Select
register_bitfields! {u64,
    pub SPSEL [
        /// Stack pointer to use.
        SP OFFSET(0) NUMBITS(1) [
            EL0 = 0,
            ELx = 1
        ]
    ]
}

pub struct Spsel;

impl Writeable for Spsel {
    type T = u64;
    type R = SPSEL::Register;

    #[inline]
    fn set(&self, value: Self::T) {
        unsafe {
            core::arch::asm!(
                "msr spsel, {}",
                in(reg) value,
                options(nomem, nostack)
            );
        }
    }
}

pub const SPSEL: Spsel = Spsel {};
