/// Enumeration of possible methods to seek within an I/O object. some as [`std::io::SeekFrom`].
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SeekFrom {
    /// Sets the offset to the provided number of bytes. as SEEK_SET.
    Start(u64),
    /// Sets the offset to the size of this object plus the specified number of bytes. as SEEK_END.
    End(i64),
    /// Sets the offset to the current position plus the specified number of bytes. as SEEK_CUR.
    Current(i64),
}

/// Maximum bytes in a file name
pub const NAME_MAX: usize = 255;
