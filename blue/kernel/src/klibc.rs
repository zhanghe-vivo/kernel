use core::{intrinsics::compare_bytes, ptr};

use crate::str::CStringIter;

/// weak *mut u8 memset(*mut u8 s, u8 c, usize count)
#[linkage = "weak"]
pub unsafe fn memset(s: *mut u8, c: u8, count: usize) -> *mut u8 {
    // write_bytes is similar to C’s memset, but sets count * size_of::<T>() bytes to val
    ptr::write_bytes(s, c, count);
    s
}

/// weak *mut u8 memcpy(*mut u8 dst, *const u8 src, usize count)
#[linkage = "weak"]
pub unsafe fn memcpy(dst: *mut u8, src: *const u8, count: usize) -> *mut u8 {
    // copy_nonoverlapping is semantically equivalent to C’s memcpy,
    // but with the argument order swapped.
    ptr::copy_nonoverlapping(src, dst, count);
    dst
}

/// *mut u8 memmove(*mut u8 dest, *const u8 src, usize n)
pub unsafe fn memmove(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    // copy is semantically equivalent to C’s memmove, but with the argument order swapped.
    // Copying takes place as if the bytes were copied from src to a temporary array
    // and then copied from the array to dst.
    ptr::copy(src, dest, n);
    dest
}

/// i32 memcmp(*const u8 cs, *const u8 ct, usize count)
pub unsafe fn memcmp(cs: *const u8, ct: *const u8, n: usize) -> i32 {
    // The compiler-builtins provides optimized versions of mem functions.
    compare_bytes(cs, ct, n)
}

pub unsafe fn memchr(s: *const u8, c: i32, n: usize) -> *const u8 {
    let s = s as *const i32;
    for i in 0..n {
        if *s.add(i) as i32 == c {
            return s.add(i) as *const u8;
        }
    }
    core::ptr::null()
}

/// char *rt_strstr(const char *s1, const char *s2)
pub unsafe fn strstr(cs: *const i8, ct: *const i8) -> *mut i8 {
    if *ct.offset(0) == 0 {
        return cs as *mut i8;
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
            return cs_trim as *mut i8;
        }
    }
    core::ptr::null_mut()
}

/// rt_int32_t rt_strcasecmp(const char *a, const char *b)
pub unsafe fn strcasecmp(s1: *const u8, s2: *const u8) -> u8 {
    let mut i = 0;
    loop {
        let s1_i = s1.add(i);
        let s2_i = s2.add(i);

        let val = (*s1_i as u8 as char).to_ascii_lowercase() as u8
            - (*s2_i as u8 as char).to_ascii_lowercase() as u8;
        if val != 0 || *s1_i == 0 {
            return val;
        }
        i += 1;
    }
}

/// char *rt_strncpy(char *dst, const char *src, rt_size_t n)
pub unsafe fn strncpy(dst: *mut i8, src: *const i8, n: usize) -> *mut i8 {
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
pub unsafe fn strcpy(dst: *mut i8, src: *const i8) -> *mut i8 {
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
pub unsafe fn strncmp(cs: *const i8, ct: *const i8, n: usize) -> i8 {
    for i in 0..n as isize {
        let cs_i = cs.offset(i);
        let ct_i = ct.offset(i);

        let val = *cs_i as i8 - *ct_i as i8;
        if val != 0 || *cs_i == 0 {
            return val;
        }
    }
    0
}

/// rt_int32_t rt_strcmp(const char *cs, const char *ct)
pub unsafe fn strcmp(cs: *const i8, ct: *const i8) -> i8 {
    for i in 0.. {
        let cs_i = cs.offset(i);
        let ct_i = ct.offset(i);

        let val = *cs_i as i8 - *ct_i as i8;
        if val != 0 || *cs_i == 0 {
            return val;
        }
    }
    0
}

/// rt_size_t rt_strlen(const char *s)
pub unsafe fn strlen(cs: *const i8) -> usize {
    let mut len = 0;
    let mut s = cs;
    while *s != 0 {
        s = s.offset(1);
        len += 1;
    }
    len
}

///rt_size_t rt_strnlen(const char *s, rt_ubase_t maxlen)
pub unsafe fn strnlen(cs: *const i8, maxlen: usize) -> usize {
    let mut len = 0;
    let mut s = cs;
    while *s != 0 && len <= maxlen {
        s = s.offset(1);
        len += 1;
    }
    len
}

/// char *rt_strdup(const char *s)
#[cfg(feature = "heap")]
pub unsafe fn strdup(cs: *const usize) -> *mut usize {
    let len = strlen(cs as *const i8) + 1;
    let tmp = crate::allocator::malloc(len);

    if !tmp.is_null() {
        memcpy(tmp, cs as *const u8, len);
    }
    tmp as *mut usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memset() {
        let mut array: [u8; 4] = [0; 4];
        unsafe {
            memset(array.as_mut_ptr(), b'a', 2);
        }
        assert_eq!(array, [b'a', b'a', 0, 0]);
    }

    #[test]
    fn test_strcasecmp() {
        let a = b"abc\0";
        let b = b"AbC\0";
        let result = unsafe { strncasecmp(a.as_ptr(), b.as_ptr()) };
        // Match!
        assert_eq!(result, 0);

        let a = b"123\0";
        let b = b"x12\0";
        let result = unsafe { strncasecmp(a.as_ptr(), b.as_ptr()) };
        // No match, first string first
        assert!(result < 0);

        let a = b"bbb\0";
        let b = b"aaa\0";
        let result = unsafe { strncasecmp(a.as_ptr(), b.as_ptr(), 3) };
        // No match, second string first
        assert!(result > 0);
    }

    #[test]
    fn test_strncmp() {
        let a = b"123\0";
        let b = b"1234\0";
        let result = unsafe { strncmp(a.as_ptr(), b.as_ptr(), 3) };
        // Match!
        assert_eq!(result, 0);

        let a = b"123\0";
        let b = b"x1234\0";
        let result = unsafe { strncmp(a.as_ptr(), b.as_ptr(), 3) };
        // No match, first string first
        assert!(result < 0);

        let a = b"bbbbb\0";
        let b = b"aaaaa\0";
        let result = unsafe { strncmp(a.as_ptr(), b.as_ptr(), 3) };
        // No match, second string first
        assert!(result > 0);
    }

    #[test]
    fn test_strlen() {
        assert_eq!(unsafe { strlen(b"\0" as *const CChar) }, 0);
        assert_eq!(unsafe { strlen(b"Blue\0" as *const CChar) }, 4);
    }

    #[test]
    fn test_strstr_no() {
        let needle = b"abcd\0".as_ptr();
        let haystack = b"efghi\0".as_ptr();
        let result = unsafe { strstr(haystack, needle) };
        assert_eq!(result, core::ptr::null());
    }

    #[test]
    fn test_strstr_start() {
        let needle = b"abc\0".as_ptr();
        let haystack = b"abcdefghi\0".as_ptr();
        let result = unsafe { strstr(haystack, needle) };
        assert_eq!(result, haystack);
    }

    #[test]
    fn test_strstr_middle() {
        let needle = b"def\0".as_ptr();
        let haystack = b"abcdefghi\0".as_ptr();
        let result = unsafe { strstr(haystack, needle) };
        assert_eq!(result, unsafe { haystack.offset(2) });
    }

    #[test]
    fn test_strstr_end() {
        let needle = b"ghi\0".as_ptr();
        let haystack = b"abcdefghi\0".as_ptr();
        let result = unsafe { strstr(haystack, needle) };
        assert_eq!(result, unsafe { haystack.offset(3) });
    }

    #[test]
    fn test_strstr_partial() {
        let needle = b"abcdefghij\0".as_ptr();
        let haystack = b"abcdefghi\0".as_ptr();
        let result = unsafe { strstr(haystack, needle) };
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
