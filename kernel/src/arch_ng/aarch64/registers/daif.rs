use tock_registers::{
    interfaces::{Readable, Writeable},
    register_bitfields,
};

// See: https://developer.arm.com/documentation/ddi0601/2024-12/AArch64-Registers/DAIF--Interrupt-Mask-Bits
register_bitfields! {u64,
    /// DAIF (Interrupt Mask Bits) Register
    pub DAIF [
         /// Debug mask bit
        D OFFSET(9) NUMBITS(1) [
            Unmasked = 0,
            Masked = 1
        ],

        /// SError interrupt mask bit
        A OFFSET(8) NUMBITS(1) [
            Unmasked = 0,
            Masked = 1
        ],

        /// IRQ mask bit
        I OFFSET(7) NUMBITS(1) [
            Unmasked = 0,
            Masked = 1
        ],

        /// FIQ mask bit
        F OFFSET(6) NUMBITS(1) [
            Unmasked = 0,
            Masked = 1
        ]
    ]
}

pub struct Daif;

impl Readable for Daif {
    type T = u64;
    type R = DAIF::Register;

    #[inline]
    fn get(&self) -> Self::T {
        let value;
        unsafe {
            core::arch::asm!(
                "mrs {}, daif",
                out(reg) value,
                options(nomem, nostack)
            );
        }
        value
    }
}

impl Writeable for Daif {
    type T = u64;
    type R = DAIF::Register;

    #[inline]
    fn set(&self, value: Self::T) {
        unsafe {
            core::arch::asm!(
                "msr daif, {}",
                in(reg) value,
                options(nomem, nostack)
            );
        }
    }
}

pub const DAIF: Daif = Daif {};
