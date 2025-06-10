//! vfs_traits.rs
#![allow(dead_code)]

use crate::{
    error::Error,
    vfs::{
        vfs_dirent::*,
        vfs_node::{InodeAttr, InodeNo},
    },
};
use alloc::{sync::Arc, vec::Vec};
use core::any::Any;

/// File operation trait
/// File operation trait
pub trait FileOperationTrait: Send + Sync {
    /// Open a file or directory
    ///
    /// # Preconditions
    /// - File/directory must be created via create_inode
    /// - File system must be mounted
    fn open(&self, path: &str, flags: i32) -> Result<InodeNo, Error>;

    fn close(&self, inode_no: InodeNo) -> Result<(), Error>;

    fn read(&self, inode_no: InodeNo, buf: &mut [u8], offset: &mut usize) -> Result<usize, Error>;

    fn write(&self, inode_no: InodeNo, buf: &[u8], offset: &mut usize) -> Result<usize, Error>;

    fn get_offset(&self, inode_no: InodeNo) -> Result<usize, Error>;

    fn seek(&self, inode_no: InodeNo, offset: usize, whence: i32) -> Result<usize, Error>;

    fn size(&self, inode_no: InodeNo) -> Result<usize, Error>;

    fn flush(&self, inode_no: InodeNo) -> Result<(), Error>;

    fn fsync(&self, inode_no: InodeNo) -> Result<(), Error>;

    fn truncate(&self, inode_no: InodeNo, size: usize) -> Result<(), Error>;

    /// Get directory entries
    ///
    /// # Parameters
    /// * `inode_no` - Inode number of the directory
    /// * `offset` - Directory entry index offset, used to continue from previous read position
    /// * `dirents` - Mutable vector to store directory entries
    /// * `count` - Maximum number of directory entries to read
    ///
    /// # Return value
    /// * Returns the actual number of directory entries read on success
    /// * Returns negative error code on failure
    /// * Returns 0 when reaching end of directory
    ///
    /// # Notes
    /// - The dirents vector will be cleared and filled with new entries
    /// - Each directory entry contains file type (d_type) and name
    /// - offset represents the directory entry index, starting from 0
    /// - Implementation should ensure returned entries count does not exceed count
    fn getdents(
        &self,
        inode_no: InodeNo,
        offset: usize,
        dirents: &mut Vec<Dirent>,
        count: usize,
    ) -> Result<usize, Error>;
}

/// File system trait
pub trait FileSystemTrait: Send + Sync {
    fn mount(
        &self,
        source: &str,
        target: &str,
        flags: u64,
        data: Option<&[u8]>,
    ) -> Result<(), Error>;

    fn unmount(&self, target: &str) -> Result<(), Error>;

    /// Create a file or directory
    ///
    /// # Parameters
    /// * `path` - Complete path of the file or directory to create
    /// * `mode` - File mode (permissions and type)
    ///
    /// # Returns
    /// * `Ok(InodeAttr)` - Creation successful, returns file attributes
    /// * `Err(i32)` - Error code
    fn create_inode(&self, path: &str, mode: u32) -> Result<InodeAttr, Error>;

    /// Delete a file or directory
    ///
    /// # Parameters
    /// * `path` - Path of the file or directory to delete
    ///
    /// # Returns
    /// * `Ok(())` - Deletion successful
    /// * `Err(i32)` - Error code
    fn remove_inode(&self, path: &str) -> Result<(), Error>;

    fn free_inode(&self, inode_no: InodeNo) -> Result<(), Error>;

    fn sync(&self) -> Result<(), Error>;

    fn lookup_path(&self, path: &str) -> Result<InodeNo, Error>;
}

/// Combined trait representing a complete file system implementation
pub trait VfsOperations: FileSystemTrait + FileOperationTrait + Any + Send + Sync {
    fn as_any(&self) -> &(dyn Any + 'static);
}

// Create a wrapper type that directly uses dyn VfsOperations
pub(crate) struct FileOpsWrapper(Arc<dyn VfsOperations>);

impl FileOperationTrait for FileOpsWrapper {
    fn open(&self, path: &str, flags: i32) -> Result<InodeNo, Error> {
        self.0.open(path, flags)
    }

    fn close(&self, inode_no: InodeNo) -> Result<(), Error> {
        self.0.close(inode_no)
    }

    fn read(&self, inode_no: InodeNo, buf: &mut [u8], offset: &mut usize) -> Result<usize, Error> {
        self.0.read(inode_no, buf, offset)
    }

    fn write(&self, inode_no: InodeNo, buf: &[u8], offset: &mut usize) -> Result<usize, Error> {
        self.0.write(inode_no, buf, offset)
    }

    fn get_offset(&self, inode_no: InodeNo) -> Result<usize, Error> {
        self.0.get_offset(inode_no)
    }

    fn seek(&self, inode_no: InodeNo, offset: usize, whence: i32) -> Result<usize, Error> {
        self.0.seek(inode_no, offset, whence)
    }

    fn size(&self, inode_no: InodeNo) -> Result<usize, Error> {
        self.0.size(inode_no)
    }

    fn flush(&self, inode_no: InodeNo) -> Result<(), Error> {
        self.0.flush(inode_no)
    }

    fn fsync(&self, inode_no: InodeNo) -> Result<(), Error> {
        self.0.fsync(inode_no)
    }

    fn truncate(&self, inode_no: InodeNo, size: usize) -> Result<(), Error> {
        self.0.truncate(inode_no, size)
    }

    fn getdents(
        &self,
        inode_no: InodeNo,
        offset: usize,
        dirents: &mut Vec<Dirent>,
        count: usize,
    ) -> Result<usize, Error> {
        self.0.getdents(inode_no, offset, dirents, count)
    }
}

// Provide conversion function for Arc<dyn VfsOperations>
pub fn as_file_ops(fs: Arc<dyn VfsOperations>) -> Arc<dyn FileOperationTrait> {
    Arc::new(FileOpsWrapper(fs))
}

// Auto-implement VfsOperations
impl<T: FileSystemTrait + FileOperationTrait + Any + Send + Sync + 'static> VfsOperations for T {
    fn as_any(&self) -> &(dyn Any + 'static) {
        self
    }
}
