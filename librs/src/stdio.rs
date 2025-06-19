use libc::{c_char, c_int};

#[no_mangle]
pub unsafe extern "C" fn puts(s: *const c_char) -> c_int {
    // FIXME: STDOUT is shared, we have to acquire a lock if we are
    // writing s and "\n" seperately.  For now, we allocate a buffer
    // to contain concat of s and "\n".
    let l = crate::string::strlen(s) as usize;
    let slice = unsafe { core::slice::from_raw_parts(s, l) };
    let mut buf = slice.to_vec();
    buf.push(b'\n' as i8);
    let rc = crate::unistd::write(1, buf.as_ptr() as *const u8, buf.len());
    if rc < 0 {
        return rc as c_int;
    }
    rc as c_int
}
