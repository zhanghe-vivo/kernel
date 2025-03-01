use bluekernel_infra::klibc;
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
    klibc::memcpy(dst, src, count)
}

/// void *rt_memmove(void *dest, const void *src, rt_size_t n)
#[no_mangle]
pub unsafe extern "C" fn rt_memmove(
    dest: *mut c_void,
    src: *const c_void,
    n: usize,
) -> *mut c_void {
    klibc::memmove(dest, src, n)
}

/// rt_int32_t rt_memcmp(const void *cs, const void *ct, rt_size_t count)
#[no_mangle]
pub unsafe extern "C" fn rt_memcmp(cs: *const c_void, ct: *const c_void, n: usize) -> c_int {
    klibc::memcmp(cs, ct, n)
}

#[no_mangle]
pub unsafe extern "C" fn rt_memchr(s: *const c_void, c: i32, n: usize) -> *const c_void {
    klibc::memchr(s, c, n)
}

/// char *rt_strstr(const char *s1, const char *s2)
#[no_mangle]
pub unsafe extern "C" fn rt_strstr(cs: *const c_char, ct: *const c_char) -> *mut c_char {
    klibc::strstr(cs, ct)
}

/// rt_int32_t rt_strcasecmp(const char *a, const char *b)
#[no_mangle]
pub unsafe extern "C" fn rt_strcasecmp(s1: *const c_char, s2: *const c_char) -> c_int {
    klibc::strcasecmp(s1, s2)
}

/// char *rt_strncpy(char *dst, const char *src, rt_size_t n)
#[no_mangle]
pub unsafe extern "C" fn rt_strncpy(
    dst: *mut c_char,
    src: *const c_char,
    n: c_size_t,
) -> *mut c_char {
    klibc::strncpy(dst, src, n)
}

/// char *rt_strcpy(char *dst, const char *src)
#[no_mangle]
pub unsafe extern "C" fn rt_strcpy(dst: *mut c_char, src: *const c_char) -> *mut c_char {
    klibc::strcpy(dst, src)
}

/// rt_int32_t rt_strncmp(const char *cs, const char *ct, rt_size_t count)
#[no_mangle]
pub unsafe extern "C" fn rt_strncmp(cs: *const c_char, ct: *const c_char, n: c_size_t) -> c_int {
    klibc::strncmp(cs, ct, n)
}

/// rt_int32_t rt_strcmp(const char *cs, const char *ct)
#[no_mangle]
pub unsafe extern "C" fn rt_strcmp(cs: *const c_char, ct: *const c_char) -> c_int {
    klibc::strcmp(cs, ct)
}

/// rt_size_t rt_strlen(const char *s)
#[no_mangle]
pub unsafe extern "C" fn rt_strlen(cs: *const c_char) -> c_size_t {
    klibc::strlen(cs)
}

///rt_size_t rt_strnlen(const char *s, rt_ubase_t maxlen)
#[no_mangle]
pub unsafe extern "C" fn rt_strnlen(cs: *const c_char, maxlen: c_size_t) -> c_size_t {
    klibc::strnlen(cs, maxlen)
}

/// char *rt_strdup(const char *s)
#[no_mangle]
pub unsafe extern "C" fn rt_strdup(cs: *const c_char) -> *mut c_char {
    klibc::strdup(cs)
}
