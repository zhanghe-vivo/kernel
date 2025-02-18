#![allow(dead_code)]
use crate::{
    error::{code, Error},
    klibc,
};
use alloc::vec::Vec;
use core::{
    alloc::AllocError,
    ffi::CStr,
    fmt::{self, Write},
    ops::Deref,
};

/// Creates a new [`CStr`] from a string literal.
///
/// The string literal should not contain any `NUL` bytes.
///
/// # Examples
///
/// ```
/// # use kernel::c_str;
/// # use core::ffi::CStr;
/// const MY_CSTR: &CStr = c_str!("My awesome CStr!");
/// ```
#[macro_export]
macro_rules! c_str {
    ($str:expr) => {{
        const S: &str = concat!($str, "\0");
        const C: &core::ffi::CStr = match core::ffi::CStr::from_bytes_with_nul(S.as_bytes()) {
            Ok(v) => v,
            Err(_) => panic!("string contains interior NUL"),
        };
        C
    }};
}

/// Allows formatting of [`fmt::Arguments`] into a raw buffer.
///
/// It does not fail if callers write past the end of the buffer so that they can calculate the
/// size required to fit everything.
///
/// # Invariants
///
/// The memory region between `pos` (inclusive) and `end` (exclusive) is valid for writes if `pos`
/// is less than `end`.
pub(crate) struct RawFormatter {
    // Use `usize` to use `saturating_*` functions.
    beg: usize,
    pos: usize,
    end: usize,
}

impl RawFormatter {
    /// Creates a new instance of [`RawFormatter`] with an empty buffer.
    fn new() -> Self {
        // INVARIANT: The buffer is empty, so the region that needs to be writable is empty.
        Self {
            beg: 0,
            pos: 0,
            end: 0,
        }
    }

    /// Creates a new instance of [`RawFormatter`] with the given buffer pointers.
    ///
    /// # Safety
    ///
    /// If `pos` is less than `end`, then the region between `pos` (inclusive) and `end`
    /// (exclusive) must be valid for writes for the lifetime of the returned [`RawFormatter`].
    pub(crate) unsafe fn from_ptrs(pos: *mut u8, end: *mut u8) -> Self {
        // INVARIANT: The safety requirements guarantee the type invariants.
        Self {
            beg: pos as _,
            pos: pos as _,
            end: end as _,
        }
    }

    /// Creates a new instance of [`RawFormatter`] with the given buffer.
    ///
    /// # Safety
    ///
    /// The memory region starting at `buf` and extending for `len` bytes must be valid for writes
    /// for the lifetime of the returned [`RawFormatter`].
    pub(crate) unsafe fn from_buffer(buf: *mut u8, len: usize) -> Self {
        let pos = buf as usize;
        // INVARIANT: We ensure that `end` is never less then `buf`, and the safety requirements
        // guarantees that the memory region is valid for writes.
        Self {
            pos,
            beg: pos,
            end: pos.saturating_add(len),
        }
    }

    /// Returns the current insert position.
    ///
    /// N.B. It may point to invalid memory.
    pub(crate) fn pos(&self) -> *mut u8 {
        self.pos as _
    }

    /// Returns the number of bytes written to the formatter.
    pub(crate) fn bytes_written(&self) -> usize {
        self.pos - self.beg
    }
}

impl fmt::Write for RawFormatter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        // `pos` value after writing `len` bytes. This does not have to be bounded by `end`, but we
        // don't want it to wrap around to 0.
        let pos_new = self.pos.saturating_add(s.len());

        // Amount that we can copy. `saturating_sub` ensures we get 0 if `pos` goes past `end`.
        let len_to_copy = core::cmp::min(pos_new, self.end).saturating_sub(self.pos);

        if len_to_copy > 0 {
            // SAFETY: If `len_to_copy` is non-zero, then we know `pos` has not gone past `end`
            // yet, so it is valid for write per the type invariants.
            unsafe {
                core::ptr::copy_nonoverlapping(
                    s.as_bytes().as_ptr(),
                    self.pos as *mut u8,
                    len_to_copy,
                )
            };
        }

        self.pos = pos_new;
        Ok(())
    }
}

/// Allows formatting of [`fmt::Arguments`] into a raw buffer.
///
/// Fails if callers attempt to write more than will fit in the buffer.
pub(crate) struct Formatter(RawFormatter);

impl Formatter {
    /// Creates a new instance of [`Formatter`] with the given buffer.
    ///
    /// # Safety
    ///
    /// The memory region starting at `buf` and extending for `len` bytes must be valid for writes
    /// for the lifetime of the returned [`Formatter`].
    pub(crate) unsafe fn from_buffer(buf: *mut u8, len: usize) -> Self {
        // SAFETY: The safety requirements of this function satisfy those of the callee.
        Self(unsafe { RawFormatter::from_buffer(buf, len) })
    }
}

impl Deref for Formatter {
    type Target = RawFormatter;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Write for Formatter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.0.write_str(s)?;

        // Fail the request if we go past the end of the buffer.
        if self.0.pos > self.0.end {
            Err(fmt::Error)
        } else {
            Ok(())
        }
    }
}

/// An owned string that is guaranteed to have exactly one `NUL` byte, which is at the end.
///
/// Used for interoperability with kernel APIs that take C strings.
///
/// # Invariants
///
/// The string is always `NUL`-terminated and contains no other `NUL` bytes.
///
/// # Examples
///
/// ```
/// use kernel::{str::CString, fmt};
///
/// let s = CString::try_from_fmt(fmt!("{}{}{}", "abc", 10, 20)).unwrap();
/// assert_eq!(s.to_bytes_with_nul(), b"abc1020\0");
///
/// let tmp = "testing";
/// let s = CString::try_from_fmt(fmt!("{tmp}{}", 123)).unwrap();
/// assert_eq!(s.to_bytes_with_nul(), b"testing123\0");
///
/// // This fails because it has an embedded `NUL` byte.
/// let s = CString::try_from_fmt(fmt!("a\0b{}", 123));
/// assert_eq!(s.is_ok(), false);
/// ```
pub struct CString {
    buf: Vec<u8>,
}

impl CString {
    /// Creates an instance of [`CString`] from the given formatted arguments.
    pub fn try_from_fmt(args: fmt::Arguments<'_>) -> Result<Self, Error> {
        // Calculate the size needed (formatted string plus `NUL` terminator).
        let mut f = RawFormatter::new();
        f.write_fmt(args)?;
        f.write_str("\0")?;
        let size = f.bytes_written();

        // Allocate a vector with the required number of bytes, and write to it.
        let mut buf = Vec::with_capacity(size);
        // SAFETY: The buffer stored in `buf` is at least of size `size` and is valid for writes.
        let mut f = unsafe { Formatter::from_buffer(buf.as_mut_ptr(), size) };
        f.write_fmt(args)?;
        f.write_str("\0")?;

        // SAFETY: The number of bytes that can be written to `f` is bounded by `size`, which is
        // `buf`'s capacity. The contents of the buffer have been initialised by writes to `f`.
        unsafe { buf.set_len(f.bytes_written()) };

        // Check that there are no `NUL` bytes before the end.
        // SAFETY: The buffer is valid for read because `f.bytes_written()` is bounded by `size`
        // (which the minimum buffer size) and is non-zero (we wrote at least the `NUL` terminator)
        // so `f.bytes_written() - 1` doesn't underflow.
        let ptr = unsafe { klibc::memchr(buf.as_ptr().cast(), 0, (f.bytes_written() - 1) as _) };
        if !ptr.is_null() {
            return Err(code::EINVAL);
        }

        // INVARIANT: We wrote the `NUL` terminator and checked above that no other `NUL` bytes
        // exist in the buffer.
        Ok(Self { buf })
    }
}

impl Deref for CString {
    type Target = CStr;

    fn deref(&self) -> &Self::Target {
        // SAFETY: The type invariants guarantee that the string is `NUL`-terminated and that no
        // other `NUL` bytes exist.
        unsafe { CStr::from_bytes_with_nul_unchecked(self.buf.as_slice()) }
    }
}

impl<'a> TryFrom<&'a CStr> for CString {
    type Error = AllocError;

    fn try_from(cstr: &'a CStr) -> Result<CString, AllocError> {
        let mut buf = Vec::new();

        buf.extend_from_slice(cstr.to_bytes_with_nul());

        // INVARIANT: The `CStr` and `CString` types have the same invariants for
        // the string data, and we copied it over without changes.
        Ok(CString { buf })
    }
}

impl fmt::Debug for CString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

/// A convenience alias for [`core::format_args`].
#[macro_export]
macro_rules! fmt {
    ($($f:tt)*) => ( core::format_args!($($f)*) )
}

pub struct CStringIter {
    ptr: *const core::ffi::c_char,
    idx: isize,
}

impl CStringIter {
    /// Create a new iterator from a pointer to a null-terminated string. The
    /// behaviour is undefined if the string is not null-terminated.
    pub fn new(s: *const core::ffi::c_char) -> CStringIter {
        CStringIter { ptr: s, idx: 0 }
    }
}

impl core::iter::Iterator for CStringIter {
    type Item = core::ffi::c_char;
    fn next(&mut self) -> Option<Self::Item> {
        let c = unsafe { *self.ptr.offset(self.idx) };
        if c == 0 {
            None
        } else {
            self.idx += 1;
            Some(c)
        }
    }
}
