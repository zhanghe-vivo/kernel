use crate::blue_kernel::klibc;

/// rt_weak void *rt_memset(void *s, int c, rt_ubase_t count)
#[linkage = "weak"]
#[no_mangle]
pub unsafe extern "C" fn rt_memset(s: *mut u8, c: u8, count: usize) -> *mut u8 {
    klibc::memset(s, c, count)
}

/// rt_weak void *rt_memcpy(void *dst, const void *src, rt_ubase_t count)
#[linkage = "weak"]
#[no_mangle]
pub unsafe extern "C" fn rt_memcpy(dst: *mut u8, src: *const u8, count: usize) -> *mut u8 {
    klibc::memcpy(dst, src, count)
}

/// void *rt_memmove(void *dest, const void *src, rt_size_t n)
#[no_mangle]
pub unsafe extern "C" fn rt_memmove(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    klibc::memmove(dest, src, n)
}

/// rt_int32_t rt_memcmp(const void *cs, const void *ct, rt_size_t count)
#[no_mangle]
pub unsafe extern "C" fn rt_memcmp(cs: *const u8, ct: *const u8, n: usize) -> i32 {
    klibc::memcmp(cs, ct, n)
}

#[no_mangle]
pub unsafe extern "C" fn rt_memchr(s: *const u8, c: i32, n: usize) -> *const u8 {
    klibc::memchr(s, c, n)
}

/// char *rt_strstr(const char *s1, const char *s2)
#[no_mangle]
pub unsafe extern "C" fn rt_strstr(cs: *const i8, ct: *const i8) -> *mut i8 {
    klibc::strstr(cs, ct)
}

/// rt_int32_t rt_strcasecmp(const char *a, const char *b)
#[no_mangle]
pub unsafe extern "C" fn rt_strcasecmp(s1: *const u8, s2: *const u8) -> u8 {
    klibc::strcasecmp(s1, s2)
}

/// char *rt_strncpy(char *dst, const char *src, rt_size_t n)
#[no_mangle]
pub unsafe extern "C" fn rt_strncpy(dst: *mut i8, src: *const i8, n: usize) -> *mut i8 {
    klibc::strncpy(dst, src, n)
}

/// char *rt_strcpy(char *dst, const char *src)
#[no_mangle]
pub unsafe extern "C" fn rt_strcpy(dst: *mut i8, src: *const i8) -> *mut i8 {
    klibc::strcpy(dst, src)
}

/// rt_int32_t rt_strncmp(const char *cs, const char *ct, rt_size_t count)
#[no_mangle]
pub unsafe extern "C" fn rt_strncmp(cs: *const i8, ct: *const i8, n: usize) -> i8 {
    klibc::strncmp(cs, ct, n)
}

/// rt_int32_t rt_strcmp(const char *cs, const char *ct)
#[no_mangle]
pub unsafe extern "C" fn rt_strcmp(cs: *const i8, ct: *const i8) -> i8 {
    klibc::strcmp(cs, ct)
}

/// rt_size_t rt_strlen(const char *s)
#[no_mangle]
pub unsafe extern "C" fn rt_strlen(cs: *const i8) -> usize {
    klibc::strlen(cs)
}

///rt_size_t rt_strnlen(const char *s, rt_ubase_t maxlen)
#[no_mangle]
pub unsafe extern "C" fn rt_strnlen(cs: *const i8, maxlen: usize) -> usize {
    klibc::strnlen(cs, maxlen)
}

/// char *rt_strdup(const char *s)
#[cfg(feature = "heap")]
#[no_mangle]
pub unsafe extern "C" fn rt_strdup(cs: *const usize) -> *mut usize {
    klibc::strdup(cs)
}
