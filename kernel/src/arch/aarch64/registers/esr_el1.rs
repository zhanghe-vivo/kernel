use tock_registers::interfaces::Readable;

pub struct EsrEl1;

impl Readable for EsrEl1 {
    type T = u64;
    type R = ();

    #[inline]
    fn get(&self) -> Self::T {
        let esr;
        unsafe {
            core::arch::asm!(
                "mrs {}, esr_el1",
                out(reg) esr,
                options(nomem, nostack, preserves_flags)
            );
        }
        esr
    }
}

pub const ESR_EL1: EsrEl1 = EsrEl1 {};
