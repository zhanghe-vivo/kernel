use tock_registers::interfaces::{Readable, Writeable};

pub struct CntpTvalEl0;

impl Readable for CntpTvalEl0 {
    type T = u64;
    type R = ();

    #[inline]
    fn get(&self) -> Self::T {
        let value;
        unsafe {
            core::arch::asm!(
                "mrs {}, cntp_tval_el0",
                out(reg) value,
                options(nomem, nostack)
            );
        }
        value
    }
}

impl Writeable for CntpTvalEl0 {
    type T = u64;
    type R = ();

    #[inline]
    fn set(&self, value: Self::T) {
        unsafe {
            core::arch::asm!(
                "msr cntp_tval_el0, {}",
                in(reg) value,
                options(nomem, nostack)
            );
        }
    }
}

pub const CNTP_TVAL_EL0: CntpTvalEl0 = CntpTvalEl0 {};
