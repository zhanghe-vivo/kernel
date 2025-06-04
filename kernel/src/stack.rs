#![allow(dead_code)]

/// Stack management structure
#[repr(C)]
#[derive(Debug)]
pub struct Stack {
    /// Current stack pointer
    sp: *mut usize,
    /// Pointer to the lowest address of the stack
    bottom: *mut u8,
    /// Stack size
    size: usize,
}

#[cfg(feature = "stack_highwater_check")]
const STACK_MAGIC_WORD: u32 = 0xa5a5_a5a5;

impl Stack {
    /// Create a new stack object from an existing byte array with a fixed size
    pub fn new(stack: *const u8, size: usize) -> Self {
        // address and size must be 4-byte aligned
        assert_eq!((stack as usize) & 0x3, 0);
        assert_eq!(size & 0x3, 0);

        #[cfg(feature = "stack_highwater_check")]
        {
            let slice = unsafe { core::slice::from_raw_parts_mut(stack as *mut u32, size / 4) };
            slice.fill(STACK_MAGIC_WORD);
        }

        Stack {
            bottom: stack as *mut _,
            sp: unsafe { stack.offset(size as isize) as *mut usize },
            size,
        }
    }

    pub fn sp(&self) -> *mut usize {
        self.sp
    }

    pub fn set_sp(&mut self, uptr: *mut usize) {
        self.sp = uptr;
    }

    // used for hw_context_switch
    pub fn sp_ptr(&self) -> *const usize {
        &self.sp as *const *mut usize as *const usize
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
            .saturating_sub((self.sp as usize).saturating_sub(self.bottom as usize) as u32)
    }

    pub fn capacity(&self) -> u32 {
        self.size as u32
    }

    #[cfg(feature = "stack_highwater_check")]
    pub fn highwater(&self) -> usize {
        let slice = unsafe { core::slice::from_raw_parts(self.bottom as *mut u32, self.size) };

        match slice.iter().position(|&word| word != STACK_MAGIC_WORD) {
            Some(offset) => self.size - offset * 4,
            None => 0,
        }
    }

    pub fn check_overflow(&self) -> bool {
        self.usage() == 0
    }
}

impl core::fmt::Display for Stack {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Stack {{ sp: {:?}, bottom: {:?}, size: {:?}, usage: {:?} }}",
            self.sp,
            self.bottom,
            self.size,
            self.usage(),
        )
    }
}
