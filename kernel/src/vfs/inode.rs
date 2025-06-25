#![allow(dead_code)]

use crate::{
    devices::Device,
    error::{code, Error},
    vfs::{
        dirent::DirBufferReader,
        file::FileAttr,
        fs::FileSystem,
        inode_mode::{mode_t, InodeFileType, InodeMode},
    },
};
use alloc::{string::String, sync::Arc};
use core::{any::Any, fmt::Debug, time::Duration};
use log::warn;

/// Filesystem inode number
pub type InodeNo = usize;

/// Inode attributes
#[derive(Debug, Clone)]
pub struct InodeAttr {
    pub inode_no: InodeNo, // Index Node number
    pub size: usize,       // File size
    pub blk_size: usize,   // Block size
    pub blocks: usize,     // Number of blocks
    pub atime: Duration,   // Access time
    pub mtime: Duration,   // Modification time
    pub ctime: Duration,   // Creation time
    pub mode: mode_t,      // File type and access permissions
    pub nlinks: u32,       // Number of hard links
    pub uid: u32,          // User ID
    pub gid: u32,          // Group ID
}

impl InodeAttr {
    /// Create new inode attributes
    pub fn new(
        inode_no: InodeNo,
        file_type: InodeFileType,
        mode: InodeMode,
        uid: u32,
        gid: u32,
        blk_size: usize,
    ) -> Self {
        Self {
            inode_no,
            size: 0,
            blk_size,
            blocks: 0,
            atime: Default::default(),
            mtime: Default::default(),
            ctime: Default::default(),
            mode: file_type as u32 | mode.bits(),
            nlinks: if file_type == InodeFileType::Directory {
                2
            } else {
                1
            },
            uid,
            gid,
        }
    }

    pub fn ino(&self) -> InodeNo {
        self.inode_no
    }
    pub fn type_(&self) -> InodeFileType {
        InodeFileType::from(self.mode)
    }
    pub fn mode(&self) -> InodeMode {
        InodeMode::from(self.mode)
    }
    pub fn size(&self) -> usize {
        self.size
    }
    fn atime(&self) -> Duration {
        self.atime
    }
    fn set_atime(&mut self, time: Duration) {
        self.atime = time;
    }
    fn mtime(&self) -> Duration {
        self.mtime
    }
    fn set_mtime(&mut self, time: Duration) {
        self.mtime = time;
    }
    pub fn set_size(&mut self, size: usize) {
        self.size = size;
    }
}

#[allow(unused_variables)]
pub trait InodeOps: Any + Sync + Send {
    fn read_at(&self, offset: usize, buf: &mut [u8], nonblock: bool) -> Result<usize, Error> {
        warn!("read_at is not implemented");
        Err(code::EINVAL)
    }
    fn write_at(&self, offset: usize, buf: &[u8], nonblock: bool) -> Result<usize, Error> {
        warn!("write_at is not implemented");
        Err(code::EINVAL)
    }
    fn link(&self, old: &Arc<dyn InodeOps>, name: &str) -> Result<(), Error> {
        warn!("link is not implemented");
        Err(code::ENOTDIR)
    }
    fn unlink(&self, name: &str) -> Result<(), Error> {
        warn!("unlink is not implemented");
        Err(code::ENOTDIR)
    }
    fn rmdir(&self, name: &str) -> Result<(), Error> {
        warn!("rmdir is not implemented");
        Err(code::ENOTDIR)
    }
    fn rename(
        &self,
        old_name: &str,
        target: &Arc<dyn InodeOps>,
        new_name: &str,
    ) -> Result<(), Error> {
        warn!("rename is not implemented");
        Err(code::ENOTDIR)
    }
    fn getdents_at(&self, offset: usize, reader: &mut DirBufferReader) -> Result<usize, Error> {
        warn!("getdents_at is not implemented");
        Err(code::ENOTDIR)
    }
    fn create(
        &self,
        name: &str,
        type_: InodeFileType,
        mode: InodeMode,
    ) -> Result<Arc<dyn InodeOps>, Error> {
        warn!("create is not implemented");
        Err(code::EINVAL)
    }
    fn create_device(
        &self,
        name: &str,
        mode: InodeMode,
        device: Arc<dyn Device>,
    ) -> Result<Arc<dyn InodeOps>, Error> {
        warn!("create_device is not implemented");
        Err(code::EINVAL)
    }
    fn close(&self) -> Result<(), Error> {
        Ok(())
    }
    fn ioctl(&self, cmd: u32, arg: usize) -> Result<i32, Error> {
        warn!("ioctl is not implemented");
        Err(code::ENOTDIR)
    }
    fn flush(&self) -> Result<(), Error> {
        Ok(())
    }
    fn fsync(&self) -> Result<(), Error> {
        Ok(())
    }
    fn lookup(&self, name: &str) -> Result<Arc<dyn InodeOps>, Error> {
        warn!("lookup is not implemented");
        Err(code::ENOTDIR)
    }
    fn resize(&self, size: usize) -> Result<(), Error> {
        warn!("resize is not supported");
        Err(code::EINVAL)
    }
    fn fs(&self) -> Option<Arc<dyn FileSystem>>;
    fn inode_attr(&self) -> InodeAttr;
    fn file_attr(&self) -> FileAttr;
    fn type_(&self) -> InodeFileType;
    fn mode(&self) -> InodeMode;
    fn size(&self) -> usize;
    fn atime(&self) -> Duration;
    fn set_atime(&self, time: Duration);
    fn mtime(&self) -> Duration;
    fn set_mtime(&self, time: Duration);
}

impl dyn InodeOps {
    pub fn downcast_ref<T: InodeOps>(&self) -> Option<&T> {
        (self as &dyn Any).downcast_ref::<T>()
    }
}

impl Debug for dyn InodeOps {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("InodeOps")
            .field("attr", &self.inode_attr())
            .field("fs", &self.fs())
            .finish()
    }
}
