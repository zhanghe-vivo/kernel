use core::{
    ffi::{c_char, c_int, c_size_t, c_void},
    intrinsics::compare_bytes,
    ptr,
};

use crate::str::CStringIter;

/// rt_weak void *rt_memset(void *s, int c, rt_ubase_t count)
#[cfg(not(feature = "RT_KSERVICE_USING_STDLIB_MEMORY"))]
#[linkage = "weak"]
#[no_mangle]
pub unsafe extern "C" fn rt_memset(s: *mut c_void, c: c_int, count: c_size_t) -> *mut c_void {
    // write_bytes is similar to C’s memset, but sets count * size_of::<T>() bytes to val
    ptr::write_bytes(s as *mut u8, c as u8, count);
    s
}

/// rt_weak void *rt_memcpy(void *dst, const void *src, rt_ubase_t count)
#[cfg(not(feature = "RT_KSERVICE_USING_STDLIB_MEMORY"))]
#[linkage = "weak"]
#[no_mangle]
pub unsafe extern "C" fn rt_memcpy(
    dst: *mut c_void,
    src: *const c_void,
    count: c_size_t,
) -> *mut c_void {
    // copy_nonoverlapping is semantically equivalent to C’s memcpy,
    // but with the argument order swapped.
    ptr::copy_nonoverlapping(src as *const u8, dst as *mut u8, count);
    dst
}

/// void *rt_memmove(void *dest, const void *src, rt_size_t n)
#[cfg(not(feature = "RT_KSERVICE_USING_STDLIB_MEMORY"))]
#[no_mangle]
pub unsafe extern "C" fn rt_memmove(
    dest: *mut c_void,
    src: *const c_void,
    n: c_size_t,
) -> *mut c_void {
    // copy is semantically equivalent to C’s memmove, but with the argument order swapped.
    // Copying takes place as if the bytes were copied from src to a temporary array
    // and then copied from the array to dst.
    ptr::copy(src as *const u8, dest as *mut u8, n);
    dest
}

/// rt_int32_t rt_memcmp(const void *cs, const void *ct, rt_size_t count)
#[cfg(not(feature = "RT_KSERVICE_USING_STDLIB_MEMORY"))]
#[no_mangle]
pub unsafe extern "C" fn rt_memcmp(cs: *const c_void, ct: *const c_void, n: c_size_t) -> c_int {
    // The compiler-builtins provides optimized versions of mem functions.
    compare_bytes(cs as *const u8, ct as *const u8, n)
}

#[cfg(not(feature = "RT_KSERVICE_USING_STDLIB_MEMORY"))]
#[no_mangle]
pub unsafe extern "C" fn rt_memchr(s: *const c_void, c: c_int, n: c_size_t) -> *const c_void {
    let s = s as *const c_char;
    for i in 0..n {
        if *s.add(i) as c_int == c {
            return s.add(i) as *const c_void;
        }
    }
    core::ptr::null()
}

/// char *rt_strstr(const char *s1, const char *s2)
#[cfg(not(feature = "RT_KSERVICE_USING_STDLIB"))]
#[no_mangle]
pub unsafe extern "C" fn rt_strstr(cs: *const c_char, ct: *const c_char) -> *mut c_char {
    if *ct.offset(0) == 0 {
        return cs as *mut c_char;
    }
    for cs_trim in (0..).map(|idx| cs.offset(idx)) {
        if *cs_trim == 0 {
            break;
        }
        let mut len = 0;
        for (inner_idx, nec) in CStringIter::new(ct).enumerate() {
            let hsc = *cs_trim.add(inner_idx);
            if hsc != nec {
                break;
            }
            len += 1;
        }
        if *ct.offset(len) == 0 {
            return cs_trim as *mut c_char;
        }
    }
    core::ptr::null_mut()
}

/// rt_int32_t rt_strcasecmp(const char *a, const char *b)
#[cfg(not(feature = "RT_KSERVICE_USING_STDLIB"))]
#[no_mangle]
pub unsafe extern "C" fn rt_strcasecmp(s1: *const c_char, s2: *const c_char) -> c_int {
    let mut i = 0;
    loop {
        let s1_i = s1.add(i);
        let s2_i = s2.add(i);

        let val = (*s1_i as u8 as char).to_ascii_lowercase() as c_int
            - (*s2_i as u8 as char).to_ascii_lowercase() as c_int;
        if val != 0 || *s1_i == 0 {
            return val;
        }
        i += 1;
    }
}

/// char *rt_strncpy(char *dst, const char *src, rt_size_t n)
#[cfg(not(feature = "RT_KSERVICE_USING_STDLIB"))]
#[no_mangle]
pub unsafe extern "C" fn rt_strncpy(
    dst: *mut c_char,
    src: *const c_char,
    n: c_size_t,
) -> *mut c_char {
    let mut i = 0;
    while i < n {
        let c = *src.add(i);
        *dst.add(i) = c;
        i += 1;
        if c == 0 {
            break;
        }
    }
    /* NUL pad the remaining n-1 bytes */
    for j in i..n {
        *dst.add(j) = 0;
    }
    dst
}

/// char *rt_strcpy(char *dst, const char *src)
#[cfg(not(feature = "RT_KSERVICE_USING_STDLIB"))]
#[no_mangle]
pub unsafe extern "C" fn rt_strcpy(dst: *mut c_char, src: *const c_char) -> *mut c_char {
    let mut i = 0;
    loop {
        let c = *src.offset(i);
        *dst.offset(i) = c;
        if c == 0 {
            break;
        }
        i += 1;
    }
    dst
}

/// rt_int32_t rt_strncmp(const char *cs, const char *ct, rt_size_t count)
#[cfg(not(feature = "RT_KSERVICE_USING_STDLIB"))]
#[no_mangle]
pub unsafe extern "C" fn rt_strncmp(cs: *const c_char, ct: *const c_char, n: c_size_t) -> c_int {
    for i in 0..n as isize {
        let cs_i = cs.offset(i);
        let ct_i = ct.offset(i);

        let val = *cs_i as c_int - *ct_i as c_int;
        if val != 0 || *cs_i == 0 {
            return val;
        }
    }
    0
}

/// rt_int32_t rt_strcmp(const char *cs, const char *ct)
#[cfg(not(feature = "RT_KSERVICE_USING_STDLIB"))]
#[no_mangle]
pub unsafe extern "C" fn rt_strcmp(cs: *const c_char, ct: *const c_char) -> c_int {
    for i in 0.. {
        let cs_i = cs.offset(i);
        let ct_i = ct.offset(i);

        let val = *cs_i as c_int - *ct_i as c_int;
        if val != 0 || *cs_i == 0 {
            return val;
        }
    }
    0
}

/// rt_size_t rt_strlen(const char *s)
#[cfg(not(feature = "RT_KSERVICE_USING_STDLIB"))]
#[no_mangle]
pub unsafe extern "C" fn rt_strlen(cs: *const c_char) -> c_size_t {
    let mut len = 0;
    let mut s = cs;
    while *s != 0 {
        s = s.offset(1);
        len += 1;
    }
    len
}

///rt_size_t rt_strnlen(const char *s, rt_ubase_t maxlen)
#[no_mangle]
pub unsafe extern "C" fn rt_strnlen(cs: *const c_char, maxlen: c_size_t) -> c_size_t {
    let mut len = 0;
    let mut s = cs;
    while *s != 0 && len <= maxlen {
        s = s.offset(1);
        len += 1;
    }
    len
}

/// char *rt_strdup(const char *s)
#[cfg(feature = "RT_USING_HEAP")]
#[no_mangle]
pub unsafe extern "C" fn rt_strdup(cs: *const c_char) -> *mut c_char {
    let len = rt_strlen(cs) + 1;
    let tmp = crate::allocator::rt_malloc(len);

    if !tmp.is_null() {
        rt_memcpy(tmp, cs as *const c_void, len);
    }
    tmp as *mut c_char
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memset() {
        let mut array: [u8; 4] = [0; 4];
        unsafe {
            rt_memset(array.as_mut_ptr(), b'a', 2);
        }
        assert_eq!(array, [b'a', b'a', 0, 0]);
    }

    #[test]
    fn test_strcasecmp() {
        let a = b"abc\0";
        let b = b"AbC\0";
        let result = unsafe { rt_strncasecmp(a.as_ptr(), b.as_ptr()) };
        // Match!
        assert_eq!(result, 0);

        let a = b"123\0";
        let b = b"x12\0";
        let result = unsafe { rt_strncasecmp(a.as_ptr(), b.as_ptr()) };
        // No match, first string first
        assert!(result < 0);

        let a = b"bbb\0";
        let b = b"aaa\0";
        let result = unsafe { rt_strncasecmp(a.as_ptr(), b.as_ptr(), 3) };
        // No match, second string first
        assert!(result > 0);
    }

    #[test]
    fn test_strncmp() {
        let a = b"123\0";
        let b = b"1234\0";
        let result = unsafe { rt_strncmp(a.as_ptr(), b.as_ptr(), 3) };
        // Match!
        assert_eq!(result, 0);

        let a = b"123\0";
        let b = b"x1234\0";
        let result = unsafe { rt_strncmp(a.as_ptr(), b.as_ptr(), 3) };
        // No match, first string first
        assert!(result < 0);

        let a = b"bbbbb\0";
        let b = b"aaaaa\0";
        let result = unsafe { rt_strncmp(a.as_ptr(), b.as_ptr(), 3) };
        // No match, second string first
        assert!(result > 0);
    }

    #[test]
    fn test_strlen() {
        assert_eq!(unsafe { rt_strlen(b"\0" as *const CChar) }, 0);
        assert_eq!(unsafe { rt_strlen(b"Blue\0" as *const CChar) }, 4);
    }

    #[test]
    fn test_strstr_no() {
        let needle = b"abcd\0".as_ptr();
        let haystack = b"efghi\0".as_ptr();
        let result = unsafe { rt_strstr(haystack, needle) };
        assert_eq!(result, core::ptr::null());
    }

    #[test]
    fn test_strstr_start() {
        let needle = b"abc\0".as_ptr();
        let haystack = b"abcdefghi\0".as_ptr();
        let result = unsafe { rt_strstr(haystack, needle) };
        assert_eq!(result, haystack);
    }

    #[test]
    fn test_strstr_middle() {
        let needle = b"def\0".as_ptr();
        let haystack = b"abcdefghi\0".as_ptr();
        let result = unsafe { rt_strstr(haystack, needle) };
        assert_eq!(result, unsafe { haystack.offset(2) });
    }

    #[test]
    fn test_strstr_end() {
        let needle = b"ghi\0".as_ptr();
        let haystack = b"abcdefghi\0".as_ptr();
        let result = unsafe { rt_strstr(haystack, needle) };
        assert_eq!(result, unsafe { haystack.offset(3) });
    }

    #[test]
    fn test_strstr_partial() {
        let needle = b"abcdefghij\0".as_ptr();
        let haystack = b"abcdefghi\0".as_ptr();
        let result = unsafe { rt_strstr(haystack, needle) };
        assert_eq!(result, core::ptr::null());
    }

    #[test]
    fn test_strncpy() {
        let src = b"hi\0";
        let mut dest = *b"abcdef";
        let result = unsafe { tr_strncpy(dest.as_mut_ptr(), src.as_ptr(), 5) };
        // two bytes of data, 3 bytes of zeros (= 5 bytes total), plus one byte unchanged
        assert_eq!(
            unsafe { core::slice::from_raw_parts(result, 6) },
            *b"hi\0\0\0f"
        );
    }
}
