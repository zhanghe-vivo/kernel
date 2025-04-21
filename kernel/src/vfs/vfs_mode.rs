//! vfs_mode.rs  
//! File mode definitions from RT-Thread sys/stat.h  
#![allow(dead_code)]

use alloc::vec::Vec;
use core::ffi::c_int;

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub enum AccessMode {
    /// read only
    O_RDONLY = libc::O_RDONLY as u8,
    /// write only
    O_WRONLY = libc::O_WRONLY as u8,
    /// read write
    O_RDWR = libc::O_RDWR as u8,
}

impl From<AccessMode> for i32 {
    fn from(mode: AccessMode) -> Self {
        mode as i32
    }
}

impl From<AccessMode> for u32 {
    fn from(mode: AccessMode) -> Self {
        mode as u32
    }
}

impl From<i32> for AccessMode {
    fn from(mode: i32) -> Self {
        match mode & libc::O_ACCMODE {
            libc::O_RDONLY => AccessMode::O_RDONLY,
            libc::O_WRONLY => AccessMode::O_WRONLY,
            libc::O_RDWR => AccessMode::O_RDWR,
            _ => AccessMode::O_RDONLY, // Default to read-only for invalid modes
        }
    }
}

/// Check if it's a symbolic link
#[inline]
pub fn s_islnk(m: u32) -> bool {
    (m & libc::S_IFMT) == libc::S_IFLNK
}

/// Check if it's a regular file
#[inline]
pub fn s_isreg(m: u32) -> bool {
    (m & libc::S_IFMT) == libc::S_IFREG
}

/// Check if it's a directory
#[inline]
pub fn s_isdir(m: u32) -> bool {
    (m & libc::S_IFMT) == libc::S_IFDIR
}

/// Check if it's a character device
#[inline]
pub fn s_ischr(m: u32) -> bool {
    (m & libc::S_IFMT) == libc::S_IFCHR
}

/// Check if it's a block device
#[inline]
pub fn s_isblk(m: u32) -> bool {
    (m & libc::S_IFMT) == libc::S_IFBLK
}

/// Check if it's a FIFO
#[inline]
pub fn s_isfifo(m: u32) -> bool {
    (m & libc::S_IFMT) == libc::S_IFIFO
}

/// Check if it's a socket
#[inline]
pub fn s_issock(m: u32) -> bool {
    (m & libc::S_IFMT) == libc::S_IFSOCK
}

// Common mode combinations
pub const DEFAULT_FILE_MODE: u32 = libc::S_IFREG | 0o644; // rw-r--r--
pub const DEFAULT_DIR_MODE: u32 = libc::S_IFDIR | 0o755; // rwxr-xr-x
pub const DEFAULT_DEV_MODE: u32 = libc::S_IFCHR | 0o660; // rw-rw----

/// Get file type (without permission bits)
#[inline]
pub fn get_file_type(mode: u32) -> u32 {
    mode & libc::S_IFMT
}

/// Get file permissions (without type bits)
#[inline]
pub fn get_file_perm(mode: u32) -> u32 {
    mode & !libc::S_IFMT
}

#[allow(non_camel_case_types)]
pub type mode_t = u32;

/// Convert open flags to readable string for debugging
pub fn flags_to_string(flags: c_int) -> alloc::string::String {
    let mut modes = Vec::new();

    // Check access mode
    match flags & libc::O_ACCMODE {
        x if x == libc::O_RDONLY => modes.push("O_RDONLY"),
        x if x == libc::O_WRONLY => modes.push("O_WRONLY"),
        x if x == libc::O_RDWR => modes.push("O_RDWR"),
        _ => modes.push("O_UNKNOWN"),
    }

    // Check creation flags
    if flags & libc::O_CREAT != 0 {
        modes.push("O_CREAT");
    }
    if flags & libc::O_EXCL != 0 {
        modes.push("O_EXCL");
    }
    if flags & libc::O_TRUNC != 0 {
        modes.push("O_TRUNC");
    }
    if flags & libc::O_APPEND != 0 {
        modes.push("O_APPEND");
    }
    if flags & libc::O_NONBLOCK != 0 {
        modes.push("O_NONBLOCK");
    }
    if flags & libc::O_SYNC != 0 {
        modes.push("O_SYNC");
    }
    // Add directory-related flags
    if flags & libc::O_DIRECTORY != 0 {
        modes.push("O_DIRECTORY");
    }
    if flags & libc::O_NOFOLLOW != 0 {
        modes.push("O_NOFOLLOW");
    }
    if flags & libc::O_CLOEXEC != 0 {
        modes.push("O_CLOEXEC");
    }
    modes.join("|")
}

/// Check if open flags include directory flag
#[inline]
pub fn is_directory_open(flags: c_int) -> bool {
    flags & libc::O_DIRECTORY != 0
}

/// Check if symlinks should be followed
#[inline]
pub fn should_follow_symlinks(flags: c_int) -> bool {
    flags & libc::O_NOFOLLOW == 0
}

pub const SEEK_SET: i32 = 0; // Seek from start of file
pub const SEEK_CUR: i32 = 1; // Seek from current position
pub const SEEK_END: i32 = 2; // Seek from end of file
