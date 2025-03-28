use core::{
    alloc::Layout,
    ffi::{c_char, c_int, c_size_t, c_uchar, c_void},
    iter::{once, zip},
    mem::{self, MaybeUninit},
    ptr, slice,
};
extern crate alloc;

mod iter;
use iter::{NulTerminated, SrcDstPtrIter};

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/memset.html>.
#[no_mangle]
pub unsafe extern "C" fn memset(s: *mut c_void, c: c_int, n: c_size_t) -> *mut c_void {
    for i in 0..n {
        *(s as *mut u8).add(i) = c as u8;
    }
    s
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/memcpy.html>.
///
/// # Safety
/// The caller must ensure that *either*:
/// - `n` is 0, *or*
///     - `s1` is convertible to a `&mut [MaybeUninit<u8>]` with length `n`,
///       and
///     - `s2` is convertible to a `&[MaybeUninit<u8>]` with length `n`.
#[no_mangle]
pub unsafe extern "C" fn memcpy(s1: *mut c_void, s2: *const c_void, n: c_size_t) -> *mut c_void {
    // Avoid creating slices for n == 0. This is because we are required to
    // avoid UB for n == 0, even if either s1 or s2 is null, to comply with the
    // expectations of Rust's core library, as well as C2y (N3322).
    // See https://doc.rust-lang.org/core/index.html for details.
    if n != 0 {
        // SAFETY: the caller is required to ensure that the provided pointers
        // are valid. The slices are required to have a length of at most
        // isize::MAX; this implicitly ensured by requiring valid pointers to
        // two nonoverlapping slices.
        let s1_slice = unsafe { slice::from_raw_parts_mut(s1.cast::<MaybeUninit<u8>>(), n) };
        let s2_slice = unsafe { slice::from_raw_parts(s2.cast::<MaybeUninit<u8>>(), n) };

        // At this point, it may seem tempting to use
        // s1_slice.copy_from_slice(s2_slice) here, but memcpy is one of the
        // handful of symbols whose existence is assumed by Rust's core
        // library, and thus we need to be careful here not to rely on any
        // function that calls memcpy internally.
        // See https://doc.rust-lang.org/core/index.html for details.
        //
        // Instead, we check the alignment of the two slices and try to
        // identify the largest Rust primitive type that is well-aligned for
        // copying in chunks. s1_slice and s2_slice will be divided into
        // (prefix, middle, suffix), where only the "middle" part is copyable
        // using the larger primitive type.
        let s1_addr = s1.addr();
        let s2_addr = s2.addr();
        // Find the number of similar trailing bits in the two addresses to let
        // us find the largest possible chunk size
        let equal_trailing_bits_count = (s1_addr ^ s2_addr).trailing_zeros();
        let chunk_size = match equal_trailing_bits_count {
            0 => 1,
            1 => 2,
            2 => 4,
            3 => 8,
            _ => 16, // use u128 chunks for any higher alignments
        };
        let chunk_align_offset = s1.align_offset(chunk_size);
        let prefix_len = chunk_align_offset.min(n);

        // Copy "prefix" bytes
        for (s1_elem, s2_elem) in zip(&mut s1_slice[..prefix_len], &s2_slice[..prefix_len]) {
            *s1_elem = *s2_elem;
        }

        if chunk_align_offset < n {
            fn copy_chunks_and_remainder<const N: usize, T: Copy>(
                dst: &mut [MaybeUninit<u8>],
                src: &[MaybeUninit<u8>],
            ) {
                // Check sanity
                assert_eq!(N, mem::size_of::<T>());
                assert_eq!(0, N % mem::align_of::<T>());
                assert!(dst.as_mut_ptr().is_aligned_to(N));
                assert!(src.as_ptr().is_aligned_to(N));

                // Split into "middle" and "suffix"
                let (dst_chunks, dst_remainder) = dst.as_chunks_mut::<N>();
                let (src_chunks, src_remainder) = src.as_chunks::<N>();

                // Copy "middle"
                for (dst_chunk, src_chunk) in zip(dst_chunks, src_chunks) {
                    // SAFETY: the chunks are safely subsliced from s1 and
                    // s2. Alignment is ensured through the use of
                    // "align_offset", while the size of the chunks is
                    // explicitly taken to match the primitive size.
                    let dst_chunk_primitive: &mut MaybeUninit<T> =
                        unsafe { &mut *dst_chunk.as_mut_ptr().cast() };
                    let src_chunk_primitive: &MaybeUninit<T> =
                        unsafe { &*src_chunk.as_ptr().cast() };
                    *dst_chunk_primitive = *src_chunk_primitive;
                }

                // Copy "suffix"
                for (dst_elem, src_elem) in zip(dst_remainder, src_remainder) {
                    *dst_elem = *src_elem;
                }
            }

            // Copy "middle" bytes (if length is sufficient) and any remaining
            // "suffix" bytes.
            let s1_middle_and_suffix = &mut s1_slice[prefix_len..];
            let s2_middle_and_suffix = &s2_slice[prefix_len..];
            match chunk_size {
                1 => {
                    for (s1_elem, s2_elem) in zip(s1_middle_and_suffix, s2_middle_and_suffix) {
                        *s1_elem = *s2_elem;
                    }
                }
                2 => {
                    copy_chunks_and_remainder::<2, u16>(s1_middle_and_suffix, s2_middle_and_suffix)
                }
                4 => {
                    copy_chunks_and_remainder::<4, u32>(s1_middle_and_suffix, s2_middle_and_suffix)
                }
                8 => {
                    copy_chunks_and_remainder::<8, u64>(s1_middle_and_suffix, s2_middle_and_suffix)
                }
                16 => copy_chunks_and_remainder::<16, u128>(
                    s1_middle_and_suffix,
                    s2_middle_and_suffix,
                ),
                _ => unreachable!(),
            }
        }
    }

    s1
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/memmove.html>.
#[no_mangle]
pub unsafe extern "C" fn memmove(s1: *mut c_void, s2: *const c_void, n: c_size_t) -> *mut c_void {
    if s2 < s1 as *const c_void {
        // copy from end
        let mut i = n;
        while i != 0 {
            i -= 1;
            *(s1 as *mut u8).add(i) = *(s2 as *const u8).add(i);
        }
    } else {
        // copy from beginning
        let mut i = 0;
        while i < n {
            *(s1 as *mut u8).add(i) = *(s2 as *const u8).add(i);
            i += 1;
        }
    }
    s1
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/memcmp.html>.
#[no_mangle]
pub unsafe extern "C" fn memcmp(s1: *const c_void, s2: *const c_void, n: usize) -> c_int {
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

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/memchr.html>.
#[no_mangle]
pub unsafe extern "C" fn memchr(
    haystack: *const c_void,
    needle: c_int,
    len: c_size_t,
) -> *mut c_void {
    let haystack = slice::from_raw_parts(haystack as *const u8, len as usize);

    match memchr::memchr(needle as u8, haystack) {
        Some(index) => haystack[index..].as_ptr() as *mut c_void,
        None => ptr::null_mut(),
    }
}

/// char *rt_strstr(const char *s1, const char *s2)
unsafe fn inner_strstr(
    mut haystack: *const c_char,
    needle: *const c_char,
    mask: c_char,
) -> *mut c_char {
    while *haystack != 0 {
        let mut i = 0;
        loop {
            if *needle.offset(i) == 0 {
                // We reached the end of the needle, everything matches this far
                return haystack as *mut c_char;
            }
            if *haystack.offset(i) & mask != *needle.offset(i) & mask {
                break;
            }

            i += 1;
        }

        haystack = haystack.offset(1);
    }
    ptr::null_mut()
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/strstr.html>.
#[no_mangle]
pub unsafe extern "C" fn strstr(haystack: *const c_char, needle: *const c_char) -> *mut c_char {
    inner_strstr(haystack, needle, !0)
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/strcasecmp.html>
pub unsafe fn strcasecmp(s1: *const c_char, s2: *const c_char) -> c_int {
    let mut i = 0;
    loop {
        let s1_i = s1.add(i);
        let s2_i = s2.add(i);

        let val = (*s1_i as u8 as char).to_ascii_lowercase() as u8
            - (*s2_i as u8 as char).to_ascii_lowercase() as u8;
        if val != 0 || *s1_i == 0 {
            return val as c_int;
        }
        i += 1;
    }
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/strncpy.html>.
#[no_mangle]
pub unsafe extern "C" fn stpncpy(
    mut s1: *mut c_char,
    mut s2: *const c_char,
    mut n: c_size_t,
) -> *mut c_char {
    while n > 0 {
        *s1 = *s2;

        if *s1 == 0 {
            break;
        }

        n -= 1;
        s1 = s1.add(1);
        s2 = s2.add(1);
    }

    memset(s1.cast(), 0, n);

    s1
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/strncpy.html>.
#[no_mangle]
pub unsafe extern "C" fn strncpy(s1: *mut c_char, s2: *const c_char, n: c_size_t) -> *mut c_char {
    stpncpy(s1, s2, n);
    s1
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/strdup.html>.
#[no_mangle]
pub unsafe extern "C" fn strndup(s1: *const c_char, size: c_size_t) -> *mut c_char {
    let len = strnlen(s1, size);
    let layout = Layout::from_size_align(len + 1, 4).expect("REASON");

    // the "+ 1" is to account for the NUL byte
    let buffer = alloc::alloc::alloc(layout) as *mut c_char;
    if buffer.is_null() {
        // platform::ERRNO.set(ENOMEM as c_int);
        return ptr::null_mut();
    } else {
        //memcpy(buffer, s1, len)
        for i in 0..len {
            *buffer.add(i) = *s1.add(i);
        }
        *buffer.add(len) = 0;
    }

    buffer
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/strdup.html>.
#[no_mangle]
pub unsafe extern "C" fn strdup(s1: *const c_char) -> *mut c_char {
    strndup(s1, usize::MAX)
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/strcpy.html>.
#[no_mangle]
pub unsafe extern "C" fn strcpy(dst: *mut c_char, src: *const c_char) -> *mut c_char {
    let src_iter = unsafe { NulTerminated::new(src) };
    let src_dest_iter = unsafe { SrcDstPtrIter::new(src_iter.chain(once(&0)), dst) };
    for (src_item, dst_item) in src_dest_iter {
        dst_item.write(*src_item);
    }

    dst
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/strncmp.html>.
#[no_mangle]
pub unsafe extern "C" fn strncmp(s1: *const c_char, s2: *const c_char, n: c_size_t) -> c_int {
    let s1 = slice::from_raw_parts(s1 as *const c_uchar, n);
    let s2 = slice::from_raw_parts(s2 as *const c_uchar, n);

    for (&a, &b) in s1.iter().zip(s2.iter()) {
        let val = (a as c_int) - (b as c_int);
        if a != b || a == 0 {
            return val;
        }
    }

    0
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/strcmp.html>.
#[no_mangle]
pub unsafe extern "C" fn strcmp(s1: *const c_char, s2: *const c_char) -> c_int {
    strncmp(s1, s2, isize::MAX as c_size_t)
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/strlen.html>.
#[no_mangle]
pub unsafe extern "C" fn strlen(s: *const c_char) -> c_size_t {
    unsafe { NulTerminated::new(s) }.count()
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/strlen.html>.
#[no_mangle]
pub unsafe extern "C" fn strnlen(s: *const c_char, size: c_size_t) -> c_size_t {
    unsafe { NulTerminated::new(s) }.take(size).count()
}

/// See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/abort.html>.
#[no_mangle]
pub unsafe extern "C" fn abort() -> ! {
    core::intrinsics::abort();
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
    fn test_strncmp() {
        let a = b"123\0";
        let b = b"1234\0";
        let result =
            unsafe { strncmp(a.as_ptr() as *const c_char, b.as_ptr() as *const c_char, 3) };
        // Match!
        assert_eq!(result, 0);

        let a = b"123\0";
        let b = b"x1234\0";
        let result =
            unsafe { strncmp(a.as_ptr() as *const c_char, b.as_ptr() as *const c_char, 3) };
        // No match, first string first
        assert!(result < 0);

        let a = b"bbbbb\0";
        let b = b"aaaaa\0";
        let result =
            unsafe { strncmp(a.as_ptr() as *const c_char, b.as_ptr() as *const c_char, 3) };
        // No match, second string first
        assert!(result > 0);
    }

    #[test]
    fn test_strlen() {
        assert_eq!(unsafe { strlen(b"\0".as_ptr() as *const c_char) }, 0);
        assert_eq!(unsafe { strlen(b"Blue\0".as_ptr() as *const c_char) }, 4);
    }

    #[test]
    fn test_strstr_no() {
        let needle = b"abcd\0".as_ptr();
        let haystack = b"efghi\0".as_ptr();
        let result = unsafe { strstr(haystack as *const c_char, needle as *const c_char) };
        assert_eq!(result, ptr::null_mut());
    }

    #[test]
    fn test_strstr_start() {
        let needle = b"abc\0".as_ptr();
        let haystack = b"abcdefghi\0".as_ptr();
        let result = unsafe { strstr(haystack as *const c_char, needle as *const c_char) };
        assert_eq!(result, haystack as *mut c_char);
    }

    #[test]
    fn test_strstr_middle() {
        let needle = b"def\0".as_ptr();
        let haystack = b"abcdefghi\0".as_ptr();
        let result = unsafe { strstr(haystack as *const c_char, needle as *const c_char) };
        assert_eq!(result, unsafe { haystack.offset(3) as *mut c_char });
    }

    #[test]
    fn test_strstr_end() {
        let needle = b"ghi\0".as_ptr();
        let haystack = b"abcdefghi\0".as_ptr();
        let result = unsafe { strstr(haystack as *const c_char, needle as *const c_char) };
        assert_eq!(result, unsafe { haystack.offset(6) as *mut c_char });
    }

    #[test]
    fn test_strstr_partial() {
        let needle = b"abcdefghij\0".as_ptr();
        let haystack = b"abcdefghi\0".as_ptr();
        let result = unsafe { strstr(haystack as *const c_char, needle as *const c_char) };
        assert_eq!(result, ptr::null_mut());
    }

    #[test]
    fn test_strncpy() {
        let src = b"hi\0";
        let mut dest = *b"abcdef";
        let result = unsafe {
            strncpy(
                dest.as_mut_ptr() as *mut c_char,
                src.as_ptr() as *const c_char,
                5,
            )
        };
        // two bytes of data, 3 bytes of zeros (= 5 bytes total), plus one byte unchanged
        assert_eq!(
            unsafe { slice::from_raw_parts(result as *mut u8, 6) },
            *b"hi\0\0\0f"
        );
    }
}
