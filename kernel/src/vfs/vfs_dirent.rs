//! vfs_dirent.rs  
#![allow(dead_code)]

use alloc::string::String;
use spin::RwLock as SpinRwLock;

/// File type enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DirType {
    Unknown = 0, // Unknown type
    Fifo = 1,    // Named pipe
    Chr = 2,     // Character device
    Dir = 4,     // Directory
    Blk = 6,     // Block device
    Reg = 8,     // Regular file
    Lnk = 10,    // Symbolic link
    Sock = 12,   // Socket
    Wht = 14,    // Reserved
}

// For backward compatibility
pub const DT_UNKNOWN: u8 = DirType::Unknown as u8;
pub const DT_FIFO: u8 = DirType::Fifo as u8;
pub const DT_CHR: u8 = DirType::Chr as u8;
pub const DT_DIR: u8 = DirType::Dir as u8;
pub const DT_BLK: u8 = DirType::Blk as u8;
pub const DT_REG: u8 = DirType::Reg as u8;
pub const DT_LNK: u8 = DirType::Lnk as u8;
pub const DT_SOCK: u8 = DirType::Sock as u8;
pub const DT_WHT: u8 = DirType::Wht as u8;

/// Directory entry structure  
#[derive(Debug, Clone)]
pub struct Dirent {
    /// The type of the file  
    pub d_type: u8,
    /// The file name  
    pub name: String,
}

impl Default for Dirent {
    fn default() -> Self {
        Self {
            d_type: DT_UNKNOWN,
            name: String::new(),
        }
    }
}

impl Dirent {
    /// Create a new directory entry  
    pub fn new(d_type: u8, name: String) -> Self {
        Self { d_type, name }
    }

    /// Get the name as a str  
    pub fn name_as_str(&self) -> &str {
        &self.name
    }
}

/// Directory state structure  
#[derive(Default)]
pub struct DirState {
    /// Current offset in directory stream  
    pub offset: usize,
}

/// Directory structure  
#[repr(C)]
#[cfg_attr(feature = "cbindgen", no_mangle)]
pub struct Dir {
    /// Directory file descriptor  
    pub fd: i32,
    /// Directory state  
    pub state: SpinRwLock<DirState>,
}

impl Default for Dir {
    fn default() -> Self {
        Self {
            fd: -1,
            state: SpinRwLock::new(DirState::default()),
        }
    }
}
