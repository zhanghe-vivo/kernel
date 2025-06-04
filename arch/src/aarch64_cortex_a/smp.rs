use crate::{aarch64_cortex_a::Arch, arch::registers::mpidr_el1::MPIDR_EL1};
use tock_registers::interfaces::Readable;
impl Arch {
    #[inline(always)]
    pub fn core_id<T>() -> T
    where
        T: From<u8>,
    {
        const CORE_MASK: u64 = 0b11;

        T::from((MPIDR_EL1.get() & CORE_MASK) as u8)
    }
}
