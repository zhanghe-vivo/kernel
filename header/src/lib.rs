#![no_std]

pub mod syscalls {
    //! BlueOS's syscall calling convention is compatible with Linux.
    // FIXME: We should really consider stable syscall nr.
    #[repr(usize)]
    #[derive(Copy, Clone)]
    pub enum NR {
        // For test only.
        Nop,
        // For test only.
        Echo,
        GetTid,
        CreateThread,
        ExitThread,
        AtomicWait,
        AtomicWake,
        // For test only
        ClockGetTime,
        AllocMem,
        FreeMem,
        Write,
        LastNR,
    }
}

pub mod thread {
    pub const DEFAULT_STACK_SIZE: usize = 8192;
    #[cfg(not(target_arch = "aarch64"))]
    pub const STACK_ALIGN: usize = core::mem::size_of::<usize>();
    #[cfg(target_arch = "aarch64")]
    pub const STACK_ALIGN: usize = 16;

    #[repr(C)]
    pub struct CloneArgs {
        pub clone_hook: Option<fn(tid: usize, clone_args: &CloneArgs)>,
        pub entry: extern "C" fn(*mut core::ffi::c_void),
        pub arg: *mut core::ffi::c_void,
        pub stack_start: *mut u8,
        pub stack_size: usize,
    }
}
