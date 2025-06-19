//! vfs_dirent.rs  
#![allow(dead_code)]

use crate::{
    error::{code, Error},
    vfs::inode_mode::InodeFileType,
};
use alloc::string::String;
use core::mem::{size_of, transmute};

/// File type enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DirentType {
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

impl From<InodeFileType> for DirentType {
    fn from(type_: InodeFileType) -> Self {
        match type_ {
            InodeFileType::Regular => DirentType::Reg,
            InodeFileType::Directory => DirentType::Dir,
            InodeFileType::SymLink => DirentType::Lnk,
            InodeFileType::CharDevice => DirentType::Chr,
            InodeFileType::BlockDevice => DirentType::Blk,
            InodeFileType::Socket => DirentType::Sock,
            InodeFileType::Fifo => DirentType::Fifo,
            InodeFileType::Unknown => DirentType::Unknown,
        }
    }
}

impl From<u8> for DirentType {
    fn from(value: u8) -> Self {
        unsafe { transmute(value) }
    }
}

/// struct Directory as libc dirent, we only support newlib for now
#[derive(Debug, Clone)]
#[repr(C)]
pub struct Dirent {
    d_ino: u32,
    /// The type of the file  
    d_type: u8,
    /// The file name  
    name: [u8; 256],
}

impl Dirent {
    pub const SIZE: usize = size_of::<Self>();

    /// Create a new Dirent instance
    pub fn new(ino: u32, type_: DirentType, name: &str) -> Self {
        crate::static_assert!(Dirent::SIZE == size_of::<libc::dirent>());
        let mut dirent = Self {
            d_ino: ino,
            d_type: type_ as u8,
            name: [0u8; 256],
        };

        let name_bytes = name.as_bytes();
        let name_len = name_bytes.len().min(255);
        dirent.name[..name_len].copy_from_slice(&name_bytes[..name_len]);
        dirent.name[name_len] = 0;

        dirent
    }

    /// Get the inode number
    pub fn ino(&self) -> u32 {
        self.d_ino
    }

    /// Get the file type
    pub fn type_(&self) -> DirentType {
        unsafe { transmute(self.d_type) }
    }

    /// Get the file name as a string
    pub fn name(&self) -> String {
        let null_pos = self.name.iter().position(|&b| b == 0).unwrap_or(256);
        String::from_utf8_lossy(&self.name[..null_pos]).into_owned()
    }

    /// Create a Dirent from a raw buffer
    pub unsafe fn from_buf(buf: &[u8]) -> Self {
        let ptr = buf.as_ptr() as *const Self;
        ptr.read_unaligned()
    }
}

pub struct DirBufferReader<'a> {
    buf: &'a mut [u8],
    read_pos: usize,
}

impl<'a> DirBufferReader<'a> {
    pub fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, read_pos: 0 }
    }

    pub fn write_node(&mut self, ino: u32, type_: InodeFileType, name: &str) -> Result<(), Error> {
        if self.read_pos + Dirent::SIZE > self.buf.len() {
            return Err(code::ENOMEM);
        }

        let dir = Dirent::new(ino, DirentType::from(type_), name);
        let dir_bytes: &[u8] = unsafe {
            core::slice::from_raw_parts(&dir as *const Dirent as *const u8, Dirent::SIZE)
        };
        self.buf[self.read_pos..self.read_pos + Dirent::SIZE].copy_from_slice(dir_bytes);
        self.read_pos += Dirent::SIZE;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bluekernel_test_macro::test;

    #[test]
    fn test_dirent() {
        let dirent = Dirent::new(1, DirentType::Reg, "test.txt");
        assert_eq!(dirent.ino(), 1);
        assert_eq!(dirent.type_(), DirentType::Reg);
        assert_eq!(dirent.name(), "test.txt");
    }

    #[test]
    fn test_dirent_long_name() {
        let long_name = "a".repeat(300);
        let dirent = Dirent::new(1, DirentType::Reg, &long_name);
        assert_eq!(dirent.name().len(), 255);
    }

    #[test]
    fn test_dir_buffer_reader() {
        let mut buf = [0u8; 1024];
        let mut reader = DirBufferReader::new(&mut buf);

        assert!(reader
            .write_node(1, InodeFileType::Regular, "test.txt")
            .is_ok());
        assert!(reader
            .write_node(2, InodeFileType::Directory, "dir")
            .is_ok());

        // Test buffer overflow
        let mut small_buf = [0u8; 1];
        let mut reader = DirBufferReader::new(&mut small_buf);
        assert!(reader
            .write_node(1, InodeFileType::Regular, "test.txt")
            .is_err());
    }
}
