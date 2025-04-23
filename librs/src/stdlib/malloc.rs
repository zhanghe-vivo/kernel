// FIXME: We are using kernel's allocator currently. Formally, we should use mmap to implement malloc.
use bluekernel_header::syscalls::NR::{AllocMem, FreeMem};
use bluekernel_scal::bk_syscall;
use libc::{c_int, c_void, size_t, EINVAL, ENOMEM};

#[no_mangle]
pub extern "C" fn posix_memalign(ptr: *mut *mut c_void, align: size_t, size: size_t) -> c_int {
    if align % core::mem::size_of::<usize>() != 0 {
        return EINVAL;
    }
    let rc = bk_syscall!(AllocMem, ptr, size, align);
    if rc != 0 {
        return ENOMEM;
    }
    return 0;
}

#[no_mangle]
pub extern "C" fn free(ptr: *mut c_void) {
    debug_assert_eq!(bk_syscall!(FreeMem, ptr), 0);
}

#[no_mangle]
pub extern "C" fn malloc(size: usize) -> *mut c_void {
    let mut ptr: *mut c_void = core::ptr::null_mut();
    let rc = posix_memalign(
        &mut ptr as *mut *mut c_void,
        core::mem::size_of::<usize>(),
        size,
    );
    if rc != 0 {
        return core::ptr::null_mut();
    }
    return ptr;
}
