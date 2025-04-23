pub mod io;
pub mod sysconf;

#[no_mangle]
#[linkage = "weak"]
pub extern "C" fn getcwd(buf: *mut i8, size: usize) -> *mut i8 {
    if size >= 2 {
        unsafe {
            core::ptr::write(buf, b'/' as i8);
            core::ptr::write(buf.add(1), b'\0' as i8);
        }
    }
    buf
}
