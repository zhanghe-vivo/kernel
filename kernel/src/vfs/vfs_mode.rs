//! vfs_mode.rs  
//! File mode definitions from RT-Thread sys/stat.h  
#![allow(dead_code)]

use alloc::vec::Vec;
use core::ffi::c_int;
use libc::{
    O_ACCMODE, O_APPEND, O_CLOEXEC, O_CREAT, O_DIRECTORY, O_EXCL, O_NOFOLLOW, O_NONBLOCK, O_RDONLY,
    O_RDWR, O_SYNC, O_TRUNC, O_WRONLY, S_IFBLK, S_IFCHR, S_IFDIR, S_IFIFO, S_IFLNK, S_IFMT,
    S_IFREG, S_IFSOCK,
};

/// Check if it's a symbolic link
#[inline]
pub fn s_islnk(m: u32) -> bool {
    (m & S_IFMT) == S_IFLNK
}

/// Check if it's a regular file
#[inline]
pub fn s_isreg(m: u32) -> bool {
    (m & S_IFMT) == S_IFREG
}

/// Check if it's a directory
#[inline]
pub fn s_isdir(m: u32) -> bool {
    (m & S_IFMT) == S_IFDIR
}

/// Check if it's a character device
#[inline]
pub fn s_ischr(m: u32) -> bool {
    (m & S_IFMT) == S_IFCHR
}

/// Check if it's a block device
#[inline]
pub fn s_isblk(m: u32) -> bool {
    (m & S_IFMT) == S_IFBLK
}

/// Check if it's a FIFO
#[inline]
pub fn s_isfifo(m: u32) -> bool {
    (m & S_IFMT) == S_IFIFO
}

/// Check if it's a socket
#[inline]
pub fn s_issock(m: u32) -> bool {
    (m & S_IFMT) == S_IFSOCK
}

// Common mode combinations
pub const DEFAULT_FILE_MODE: u32 = S_IFREG | 0o644; // rw-r--r--
pub const DEFAULT_DIR_MODE: u32 = S_IFDIR | 0o755; // rwxr-xr-x
pub const DEFAULT_DEV_MODE: u32 = S_IFCHR | 0o660; // rw-rw----

/// Get file type (without permission bits)
#[inline]
pub fn get_file_type(mode: u32) -> u32 {
    mode & S_IFMT
}

/// Get file permissions (without type bits)
#[inline]
pub fn get_file_perm(mode: u32) -> u32 {
    mode & !S_IFMT
}

#[allow(non_camel_case_types)]
pub type mode_t = u32;

/// Convert open flags to readable string for debugging
pub fn flags_to_string(flags: c_int) -> alloc::string::String {
    let mut modes = Vec::new();

    // Check access mode
    match flags & O_ACCMODE {
        x if x == O_RDONLY => modes.push("O_RDONLY"),
        x if x == O_WRONLY => modes.push("O_WRONLY"),
        x if x == O_RDWR => modes.push("O_RDWR"),
        _ => modes.push("O_UNKNOWN"),
    }

    // Check creation flags
    if flags & O_CREAT != 0 {
        modes.push("O_CREAT");
    }
    if flags & O_EXCL != 0 {
        modes.push("O_EXCL");
    }
    if flags & O_TRUNC != 0 {
        modes.push("O_TRUNC");
    }
    if flags & O_APPEND != 0 {
        modes.push("O_APPEND");
    }
    if flags & O_NONBLOCK != 0 {
        modes.push("O_NONBLOCK");
    }
    if flags & O_SYNC != 0 {
        modes.push("O_SYNC");
    }
    // Add directory-related flags
    if flags & O_DIRECTORY != 0 {
        modes.push("O_DIRECTORY");
    }
    if flags & O_NOFOLLOW != 0 {
        modes.push("O_NOFOLLOW");
    }
    if flags & O_CLOEXEC != 0 {
        modes.push("O_CLOEXEC");
    }
    modes.join("|")
}

/// Check if open flags include directory flag
#[inline]
pub fn is_directory_open(flags: c_int) -> bool {
    flags & O_DIRECTORY != 0
}

/// Check if symlinks should be followed
#[inline]
pub fn should_follow_symlinks(flags: c_int) -> bool {
    flags & O_NOFOLLOW == 0
}

pub const SEEK_SET: i32 = 0; // Seek from start of file
pub const SEEK_CUR: i32 = 1; // Seek from current position
pub const SEEK_END: i32 = 2; // Seek from end of file
