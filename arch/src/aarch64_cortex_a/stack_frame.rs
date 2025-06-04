use core::fmt;

// In AArch64, the stack pointer SP must be 128-bit aligned.
#[repr(C)]
pub struct StackFrame {
    pub elr: u64,
    pub spsr: u64,
    pub xzr: u64,
    pub lr: u64,
    pub x28: u64,
    pub fp: u64,
    pub x26: u64,
    pub x27: u64,
    pub x24: u64,
    pub x25: u64,
    pub x22: u64,
    pub x23: u64,
    pub x20: u64,
    pub x21: u64,
    pub x18: u64,
    pub x19: u64,
    pub x16: u64,
    pub x17: u64,
    pub x14: u64,
    pub x15: u64,
    pub x12: u64,
    pub x13: u64,
    pub x10: u64,
    pub x11: u64,
    pub x8: u64,
    pub x9: u64,
    pub x6: u64,
    pub x7: u64,
    pub x4: u64,
    pub x5: u64,
    pub x2: u64,
    pub x3: u64,
    pub x0: u64,
    pub x1: u64,
}

impl fmt::Display for StackFrame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "elr:  {:#018x}\n", self.elr)?;
        write!(f, "spsr: {:#018x}\n", self.spsr)?;
        write!(f, "lr:   {:#018x}\n", self.lr)?;
        write!(f, "xzr:  {:#018x}\n", self.xzr)?;
        write!(f, "x28:  {:#018x}\n", self.x28)?;
        write!(f, "fp:   {:#018x}\n", self.fp)?;
        write!(f, "x26:  {:#018x}\n", self.x26)?;
        write!(f, "x27:  {:#018x}\n", self.x27)?;
        write!(f, "x24:  {:#018x}\n", self.x24)?;
        write!(f, "x25:  {:#018x}\n", self.x25)?;
        write!(f, "x22:  {:#018x}\n", self.x22)?;
        write!(f, "x23:  {:#018x}\n", self.x23)?;
        write!(f, "x20:  {:#018x}\n", self.x20)?;
        write!(f, "x21:  {:#018x}\n", self.x21)?;
        write!(f, "x18:  {:#018x}\n", self.x18)?;
        write!(f, "x19:  {:#018x}\n", self.x19)?;
        write!(f, "x16:  {:#018x}\n", self.x16)?;
        write!(f, "x17:  {:#018x}\n", self.x17)?;
        write!(f, "x14:  {:#018x}\n", self.x14)?;
        write!(f, "x15:  {:#018x}\n", self.x15)?;
        write!(f, "x12:  {:#018x}\n", self.x12)?;
        write!(f, "x13:  {:#018x}\n", self.x13)?;
        write!(f, "x10:  {:#018x}\n", self.x10)?;
        write!(f, "x11:  {:#018x}\n", self.x11)?;
        write!(f, "x8:   {:#018x}\n", self.x8)?;
        write!(f, "x9:   {:#018x}\n", self.x9)?;
        write!(f, "x6:   {:#018x}\n", self.x6)?;
        write!(f, "x7:   {:#018x}\n", self.x7)?;
        write!(f, "x4:   {:#018x}\n", self.x4)?;
        write!(f, "x5:   {:#018x}\n", self.x5)?;
        write!(f, "x2:   {:#018x}\n", self.x2)?;
        write!(f, "x3:   {:#018x}\n", self.x3)?;
        write!(f, "x0:   {:#018x}\n", self.x0)?;
        write!(f, "x1:   {:#018x}\n", self.x1)?;
        Ok(())
    }
}
