use tock_registers::interfaces::{Readable, Writeable};

pub struct VbarEl1;

impl Readable for VbarEl1 {
    type T = u64;
    type R = ();

    #[inline]
    fn get(&self) -> Self::T {
        let value;
        unsafe {
            core::arch::asm!(
                "mrs {}, vbar_el1",
                out(reg) value,
                options(nomem, nostack)
            );
        }
        value
    }
}

impl Writeable for VbarEl1 {
    type T = u64;
    type R = ();

    #[inline]
    fn set(&self, value: Self::T) {
        unsafe {
            core::arch::asm!(
                "msr vbar_el1, {}",
                in(reg) value,
                options(nomem, nostack)
            );
        }
    }
}

pub const VBAR_EL1: VbarEl1 = VbarEl1 {};
