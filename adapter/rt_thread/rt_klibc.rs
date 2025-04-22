use bluekernel_infra::string;
use core::ffi::{c_char, c_int, c_size_t, c_void};

/// rt_weak void *rt_memset(void *s, int c, rt_ubase_t count)
#[linkage = "weak"]
#[no_mangle]
pub unsafe extern "C" fn rt_memset(s: *mut c_void, c: i32, count: usize) -> *mut c_void {
    klibc::memset(s, c, count)
}

/// rt_weak void *rt_memcpy(void *dst, const void *src, rt_ubase_t count)
#[linkage = "weak"]
#[no_mangle]
pub unsafe extern "C" fn rt_memcpy(
    dst: *mut c_void,
    src: *const c_void,
    count: usize,
) -> *mut c_void {
    string::memcpy(dst, src, count)
}

/// rt_int32_t rt_memcmp(const void *cs, const void *ct, rt_size_t count)
#[no_mangle]
pub unsafe extern "C" fn rt_memcmp(cs: *const c_void, ct: *const c_void, n: usize) -> c_int {
    string::memcmp(cs, ct, n)
}

/// char *rt_strncpy(char *dst, const char *src, rt_size_t n)
#[no_mangle]
pub unsafe extern "C" fn rt_strncpy(
    dst: *mut c_char,
    src: *const c_char,
    n: c_size_t,
) -> *mut c_char {
    string::strncpy(dst, src, n)
}

/// rt_size_t rt_strlen(const char *s)
#[no_mangle]
pub unsafe extern "C" fn rt_strlen(cs: *const c_char) -> c_size_t {
    string::strlen(cs)
}

///rt_size_t rt_strnlen(const char *s, rt_ubase_t maxlen)
#[no_mangle]
pub unsafe extern "C" fn rt_strnlen(cs: *const c_char, maxlen: c_size_t) -> c_size_t {
    string::strnlen(cs, maxlen)
}
