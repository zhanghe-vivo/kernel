use tock_registers::interfaces::Readable;

pub struct CntpctEl0;

impl Readable for CntpctEl0 {
    type T = u64;
    type R = ();

    #[inline]
    fn get(&self) -> Self::T {
        let value;
        unsafe {
            core::arch::asm!(
                "isb",
                "mrs {}, cntpct_el0",
                out(reg) value,
                options(nomem, nostack)
            );
        }
        value
    }
}

pub const CNTPCT_EL0: CntpctEl0 = CntpctEl0 {};
