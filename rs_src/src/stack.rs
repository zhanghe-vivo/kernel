#![allow(dead_code)]
use core::ptr::NonNull;

/// Stack management structure
#[repr(C)]
#[derive(Debug)]
pub struct Stack {
    /// Current stack pointer
    sp: usize,
    /// Pointer to the lowest address of the stack
    bottom: *mut u8,
    /// Stack size
    size: usize,
}

impl Stack {
    /// Create a new stack object from an existing byte array with a fixed size
    pub fn new(stack: *const u8, size: usize) -> Self {
        Stack {
            bottom: stack as *mut _,
            sp: unsafe { stack.offset(size as isize) as usize },
            size,
        }
    }

    pub fn sp(&self) -> usize {
        self.sp
    }

    pub fn set_sp(&mut self, uptr: usize) {
        self.sp = uptr;
    }

    // used for hw_context_switch
    pub fn sp_ptr(&self) -> *const usize {
        &self.sp as *const usize
    }

    /// Pointer to first element of the stack
    pub fn bottom_ptr(&self) -> *mut u8 {
        self.bottom
    }

    /// Stack size in bytes
    pub fn size(&self) -> usize {
        self.size
    }

    /// Stack usage.
    pub fn usage(&self) -> u32 {
        self.capacity()
            .saturating_sub((self.sp).saturating_sub(self.bottom as usize) as u32)
    }

    pub fn capacity(&self) -> u32 {
        self.size as u32
    }
}
