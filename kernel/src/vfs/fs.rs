#![allow(dead_code)]
use crate::{
    error::Error,
    vfs::{dcache::Dcache, inode::InodeOps},
};
use alloc::sync::Arc;
use core::{any::Any, fmt::Debug};

/// File system information, used for statfs
#[derive(Debug, Clone)]
pub struct FileSystemInfo {
    pub magic: usize,
    pub namelen: usize,
    pub bsize: usize,
    pub frsize: usize,
    pub blocks: usize,
    pub bfree: usize,
    pub bavail: usize,
    pub files: usize,
    pub ffree: usize,
    pub favail: usize,
    pub fsid: u64,
    pub flags: u64,
}

impl FileSystemInfo {
    pub fn new(magic: usize, name_max_len: usize, block_size: usize) -> Self {
        Self {
            magic,
            namelen: name_max_len,
            bsize: block_size,
            frsize: block_size,
            blocks: 0,
            bfree: 0,
            bavail: 0,
            files: 0,
            ffree: 0,
            favail: 0,
            fsid: 0,
            flags: 0,
        }
    }
}

/// File system trait
#[allow(dead_code)]
pub trait FileSystem: Any + Send + Sync {
    fn mount(&self, mount_point: Arc<Dcache>) -> Result<(), Error>;

    fn unmount(&self) -> Result<(), Error>;

    fn sync(&self) -> Result<(), Error>;

    fn root_inode(&self) -> Arc<dyn InodeOps>;

    fn fs_info(&self) -> FileSystemInfo;

    fn fs_type(&self) -> &str;
}

impl dyn FileSystem {
    pub fn downcast_ref<T: FileSystem>(&self) -> Option<&T> {
        (self as &dyn Any).downcast_ref::<T>()
    }
}

impl Debug for dyn FileSystem {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("FileSystem")
            .field("fs_info", &self.fs_info())
            .field("fs_type", &self.fs_type())
            .finish()
    }
}
