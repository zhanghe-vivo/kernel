#[derive(Debug, Copy, Clone, Eq, Ord, PartialOrd, PartialEq)]
#[repr(transparent)]
pub struct IrqNumber(usize);

impl IrqNumber {
    pub const fn new(irq: usize) -> Self {
        Self(irq)
    }
}

impl From<IrqNumber> for usize {
    fn from(irq: IrqNumber) -> Self {
        irq.0
    }
}
