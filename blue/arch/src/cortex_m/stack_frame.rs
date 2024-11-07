//! CPU Register definitions.
//!
//! Adapted from the `cortex-m` crate.
use core::fmt;

/// CPU registers pushed/popped by the hardware
#[repr(C)]
pub struct StackFrame {
    /// (General purpose) Register 0
    pub r0: u32,
    /// (General purpose) Register 1
    pub r1: u32,
    /// (General purpose) Register 2
    pub r2: u32,
    /// (General purpose) Register 3
    pub r3: u32,
    /// (General purpose) Register 12
    pub r12: u32,
    /// Link Register
    pub lr: u32,
    /// Program Counter
    pub pc: u32,
    /// Program Status Register
    pub xpsr: u32,
}

/// CPU registers the software must push/pop to/from the stack
#[repr(C)]
pub struct StackFrameExtension {
    /// (General purpose) Register 4
    pub r4: u32,
    /// (General purpose) Register 5
    pub r5: u32,
    /// (General purpose) Register 6
    pub r6: u32,
    /// (General purpose) Register 7
    pub r7: u32,
    /// (General purpose) Register 8
    pub r8: u32,
    /// (General purpose) Register 9
    pub r9: u32,
    /// (General purpose) Register 10
    pub r10: u32,
    /// (General purpose) Register 11
    pub r11: u32,
}

/// FPU registers the software must push/pop to/from the stack
#[repr(C)]
pub struct StackFrameFpu {
    /// Floating Point Register 16
    pub s16: u32,
    /// Floating Point Register 17
    pub s17: u32,
    /// Floating Point Register 18
    pub s18: u32,
    /// Floating Point Register 19
    pub s19: u32,
    /// Floating Point Register 20
    pub s20: u32,
    /// Floating Point Register 21
    pub s21: u32,
    /// Floating Point Register 22
    pub s22: u32,
    /// Floating Point Register 23
    pub s23: u32,
    /// Floating Point Register 24
    pub s24: u32,
    /// Floating Point Register 25
    pub s25: u32,
    /// Floating Point Register 26
    pub s26: u32,
    /// Floating Point Register 27
    pub s27: u32,
    /// Floating Point Register 28
    pub s28: u32,
    /// Floating Point Register 29
    pub s29: u32,
    /// Floating Point Register 30
    pub s30: u32,
    /// Floating Point Register 31
    pub s31: u32,
}

/// Additional Settings pushed to the stack
#[repr(C)]
pub struct StackSettings {
    /// Link register in exception context
    pub exception_lr: u32,
    // Control register (privilege level)
    // pub control: u32,
}

#[repr(C)]
pub struct ExceptionFrame {
    pub settings: StackSettings,
    pub extensions: StackFrameExtension,
    pub base_frame: StackFrame,
}

impl fmt::Display for ExceptionFrame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "exception_lr 0x{:x}\n\
             xpsr         0x{:x}\n\n\
             r0           0x{:x}\n\
             r1           0x{:x}\n\
             r2           0x{:x}\n\
             r3           0x{:x}\n\
             r4           0x{:x}\n\
             r5           0x{:x}\n\
             r6           0x{:x}\n\
             r7           0x{:x}\n\
             r8           0x{:x}\n\
             r9           0x{:x}\n\
             r10          0x{:x}\n\
             r11          0x{:x}\n\
             r12          0x{:x}\n\
             pc           0x{:x}\n\
             lr           0x{:x}\n",
            self.settings.exception_lr,
            self.base_frame.xpsr,
            self.base_frame.r0,
            self.base_frame.r1,
            self.base_frame.r2,
            self.base_frame.r3,
            self.extensions.r4,
            self.extensions.r5,
            self.extensions.r6,
            self.extensions.r7,
            self.extensions.r8,
            self.extensions.r9,
            self.extensions.r10,
            self.extensions.r11,
            self.base_frame.r12,
            self.base_frame.pc,
            self.base_frame.lr
        )
    }
}
