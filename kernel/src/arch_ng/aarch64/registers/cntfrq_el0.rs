use tock_registers::interfaces::Readable;

pub struct CntfrqEl0;

impl Readable for CntfrqEl0 {
    type T = u64;
    type R = ();

    #[inline]
    fn get(&self) -> Self::T {
        let value;
        unsafe {
            core::arch::asm!(
                "mrs {}, cntfrq_el0",
                out(reg) value,
                options(nomem, nostack)
            );
        }
        value
    }
}

pub const CNTFRQ_EL0: CntfrqEl0 = CntfrqEl0 {};
