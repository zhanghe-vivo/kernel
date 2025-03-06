//! vfs_mode.rs  
//! File mode definitions from RT-Thread sys/stat.h  
#![allow(dead_code)]

use alloc::vec::Vec;
use core::ffi::c_int;

// File type mask
pub const S_IFMT: u32 = 0o170000;
pub const S_IFSOCK: u32 = 0o140000;
pub const S_IFLNK: u32 = 0o120000;
pub const S_IFREG: u32 = 0o100000;
pub const S_IFBLK: u32 = 0o060000;
pub const S_IFDIR: u32 = 0o040000;
pub const S_IFCHR: u32 = 0o020000;
pub const S_IFIFO: u32 = 0o010000;

// Special permission bits
pub const S_ISUID: u32 = 0o004000;
pub const S_ISGID: u32 = 0o002000;
pub const S_ISVTX: u32 = 0o001000;

// User permissions
pub const S_IRWXU: u32 = 0o700;
pub const S_IRUSR: u32 = 0o400;
pub const S_IWUSR: u32 = 0o200;
pub const S_IXUSR: u32 = 0o100;

// Group permissions
pub const S_IRWXG: u32 = 0o070;
pub const S_IRGRP: u32 = 0o040;
pub const S_IWGRP: u32 = 0o020;
pub const S_IXGRP: u32 = 0o010;

// Other permissions
pub const S_IRWXO: u32 = 0o007;
pub const S_IROTH: u32 = 0o004;
pub const S_IWOTH: u32 = 0o002;
pub const S_IXOTH: u32 = 0o001;

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

// File access modes
pub const O_RDONLY: i32 = 0o0; // Read only
pub const O_WRONLY: i32 = 0o1; // Write only
pub const O_RDWR: i32 = 0o2; // Read and write
pub const O_ACCMODE: i32 = 0o3; // Access mode mask

// File creation flags
pub const O_CREAT: i32 = 0o100; // Create file if it doesn't exist
pub const O_EXCL: i32 = 0o200; // Error if O_CREAT and file exists
pub const O_NOCTTY: i32 = 0o400; // Don't assign controlling terminal
pub const O_TRUNC: i32 = 0o1000; // Truncate if exists
pub const O_APPEND: i32 = 0o2000; // Append mode
pub const O_NONBLOCK: i32 = 0o4000; // Non-blocking mode
pub const O_SYNC: i32 = 0o10000; // Synchronous mode

// Directory operation flags
pub const O_DIRECTORY: i32 = 0o200000; // Fail if not directory
pub const O_NOFOLLOW: i32 = 0o400000; // Don't follow symlinks
pub const O_CLOEXEC: i32 = 0o2000000; // Close on exec
pub const O_PATH: i32 = 0o10000000; // Path-only file descriptor

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
    if flags & O_NOCTTY != 0 {
        modes.push("O_NOCTTY");
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
    if flags & O_PATH != 0 {
        modes.push("O_PATH");
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
