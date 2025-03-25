//! `string.h` implementation.
//!
//! See <https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/string.h.html>.

use core::{
    ffi::{c_char, c_int, c_size_t, c_void},
    fmt, mem, ptr, slice,
};

use libc::ERANGE;

use crate::{errno::STR_ERROR, iter::NullTerminated};

pub struct StringWriter(pub *mut u8, pub usize);

impl StringWriter {
    pub fn write(&mut self, buf: &[u8]) -> Result<usize, ()> {
        if self.1 > 1 {
            let copy_size = buf.len().min(self.1 - 1);
            unsafe {
                ptr::copy_nonoverlapping(buf.as_ptr(), self.0, copy_size);
                self.1 -= copy_size;

                self.0 = self.0.add(copy_size);
                *self.0 = 0;
            }
        }

        // Pretend the entire slice was written. This is because many functions
        // (like snprintf) expects a return value that reflects how many bytes
        // *would have* been written. So keeping track of this information is
        // good, and then if we want the *actual* written size we can just go
        // `cmp::min(written, maxlen)`.
        Ok(buf.len())
    }

    pub fn write_str(&mut self, s: &str) -> fmt::Result {
        // can't fail
        self.write(s.as_bytes()).unwrap();
        Ok(())
    }

    pub fn write_u8(&mut self, byte: u8) -> fmt::Result {
        // can't fail
        self.write(&[byte]).unwrap();
        Ok(())
    }

    pub fn flush(&mut self) -> Result<(), ()> {
        Ok(())
    }
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/memccpy.html>.
#[linkage = "weak"]
#[no_mangle]
pub unsafe extern "C" fn memccpy(
    dest: *mut c_void,
    src: *const c_void,
    c: c_int,
    n: c_size_t,
) -> *mut c_void {
    let to = memchr(src, c, n);
    if to.is_null() {
        return to;
    }
    let dist = (to as usize) - (src as usize);
    if memcpy(dest, src, dist).is_null() {
        return ptr::null_mut();
    }
    (dest as *mut u8).add(dist + 1) as *mut c_void
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/memchr.html>.
#[linkage = "weak"]
#[no_mangle]
pub unsafe extern "C" fn memchr(
    haystack: *const c_void,
    needle: c_int,
    len: c_size_t,
) -> *mut c_void {
    let haystack = slice::from_raw_parts(haystack as *const u8, len as usize);

    match core::slice::memchr::memchr(needle as u8, haystack) {
        Some(index) => haystack[index..].as_ptr() as *mut c_void,
        None => ptr::null_mut(),
    }
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/memcpy.html>.
#[linkage = "weak"]
#[no_mangle]
pub unsafe extern "C" fn memcpy(s1: *mut c_void, s2: *const c_void, n: c_size_t) -> *mut c_void {
    let mut i = 0;
    while i + 7 < n {
        *(s1.add(i) as *mut u64) = *(s2.add(i) as *const u64);
        i += 8;
    }
    while i < n {
        *(s1 as *mut u8).add(i) = *(s2 as *const u8).add(i);
        i += 1;
    }
    s1
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/memset.html>.
#[linkage = "weak"]
#[no_mangle]
pub unsafe extern "C" fn memset(s: *mut c_void, c: c_int, n: c_size_t) -> *mut c_void {
    for i in 0..n {
        *(s as *mut u8).add(i) = c as u8;
    }
    s
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/memcmp.html>.
#[linkage = "weak"]
#[no_mangle]
pub unsafe extern "C" fn memcmp(s1: *const c_void, s2: *const c_void, n: c_size_t) -> c_int {
    let (div, rem) = (n / mem::size_of::<usize>(), n % mem::size_of::<usize>());
    let mut a = s1 as *const usize;
    let mut b = s2 as *const usize;
    for _ in 0..div {
        if *a != *b {
            for i in 0..mem::size_of::<usize>() {
                let c = *(a as *const u8).add(i);
                let d = *(b as *const u8).add(i);
                if c != d {
                    return c as c_int - d as c_int;
                }
            }
            unreachable!()
        }
        a = a.offset(1);
        b = b.offset(1);
    }

    let mut a = a as *const u8;
    let mut b = b as *const u8;
    for _ in 0..rem {
        if *a != *b {
            return *a as c_int - *b as c_int;
        }
        a = a.offset(1);
        b = b.offset(1);
    }
    0
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/strlen.html>.
#[linkage = "weak"]
#[no_mangle]
pub unsafe extern "C" fn strlen(s: *const c_char) -> c_size_t {
    unsafe { NullTerminated::new(s) }.count()
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/strerror.html>.
#[linkage = "weak"]
#[no_mangle]
pub unsafe extern "C" fn strerror(errnum: c_int) -> *mut c_char {
    static mut strerror_buf: [u8; 256] = [0; 256];

    let mut w = StringWriter(strerror_buf.as_mut_ptr(), strerror_buf.len());

    if errnum >= 0 && errnum < STR_ERROR.len() as c_int {
        let _ = w.write_str(STR_ERROR[errnum as usize]);
    } else {
        let _ = w.write_str("Unknown error {}");
    }

    strerror_buf.as_mut_ptr() as *mut c_char
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/strerror.html>.
#[linkage = "weak"]
#[no_mangle]
pub unsafe extern "C" fn strerror_r(errnum: c_int, buf: *mut c_char, buflen: c_size_t) -> c_int {
    let msg = strerror(errnum);
    let len = strlen(msg);

    if len >= buflen {
        if buflen != 0 {
            memcpy(buf as *mut c_void, msg as *const c_void, buflen - 1);
            *buf.add(buflen - 1) = 0;
        }
        return ERANGE as c_int;
    }
    memcpy(buf as *mut c_void, msg as *const c_void, len + 1);

    0
}

#[linkage = "weak"]
#[no_mangle]
pub unsafe extern "C" fn __xpg_strerror_r(
    errnum: c_int,
    buf: *mut c_char,
    buflen: c_size_t,
) -> c_int {
    strerror_r(errnum, buf, buflen)
}
