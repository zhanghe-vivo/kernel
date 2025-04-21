//! vfs_fd.rs  
#![allow(dead_code)]

use crate::{
    error::{code, Error},
    vfs::{vfs_log::*, vfs_mnt, vfs_node::InodeNo, vfs_traits::*},
};
use alloc::{sync::Arc, vec::Vec};
use core::ffi::c_int;
use spin::{Mutex as SpinLock, Once};

/// Standard file descriptors
pub const STDIN_FILENO: c_int = 0;
pub const STDOUT_FILENO: c_int = 1;
pub const STDERR_FILENO: c_int = 2;
/// First available file descriptor
pub const FIRST_FD: c_int = 3;
pub const MAX_FD_SIZE: c_int = 100;

/// File descriptor structure
#[derive(Clone)]
pub struct FileDescriptor {
    /// File descriptor number
    pub fd: c_int,
    /// Open flags
    pub flags: c_int,
    /// Current offset
    pub offset: usize,
    /// File operation object
    pub file: Arc<dyn FileOperationTrait>,
    /// File inode number
    pub inode_no: InodeNo,
}

/// File descriptor manager
pub struct FdManager {
    /// File descriptor table
    fds: Vec<Option<FileDescriptor>>,
    /// Next available file descriptor
    next_fd: c_int,
}

impl FdManager {
    /// Create new file descriptor manager
    pub fn new() -> Self {
        let mut manager = Self {
            fds: Vec::with_capacity(FIRST_FD as usize + MAX_FD_SIZE as usize),
            next_fd: FIRST_FD,
        };

        // Reserve standard file descriptors
        manager.fds.resize(FIRST_FD as usize, None);
        manager
    }

    pub fn init_stdio(&mut self) -> Result<(), Error> {
        let Some((fs, _)) = vfs_mnt::find_filesystem("/dev/console") else {
            vfslog!("[fd] init_stdio: Failed to find filesystem for /dev/console");
            return Err(code::ENOENT);
        };
        let stdin_inode = fs.open("console", libc::O_RDONLY)?;
        let stdout_inode = fs.open("console", libc::O_WRONLY)?;
        let stderr_inode = fs.open("console", libc::O_WRONLY)?;
        let file_ops = as_file_ops(fs);

        self.fds[STDIN_FILENO as usize] = Some(FileDescriptor {
            fd: STDIN_FILENO,
            flags: libc::O_RDONLY,
            offset: 0,
            file: file_ops.clone(),
            inode_no: stdin_inode,
        });

        self.fds[STDOUT_FILENO as usize] = Some(FileDescriptor {
            fd: STDOUT_FILENO,
            flags: libc::O_WRONLY,
            offset: 0,
            file: file_ops.clone(),
            inode_no: stdout_inode,
        });

        self.fds[STDERR_FILENO as usize] = Some(FileDescriptor {
            fd: STDERR_FILENO,
            flags: libc::O_WRONLY,
            offset: 0,
            file: file_ops.clone(),
            inode_no: stderr_inode,
        });

        Ok(())
    }

    /// Allocate new file descriptor
    pub fn alloc_fd(
        &mut self,
        flags: c_int,
        file: Arc<dyn FileOperationTrait>,
        inode_no: InodeNo,
    ) -> c_int {
        let fd = self.next_fd;

        if (fd as usize) >= self.fds.len() {
            self.fds.resize(fd as usize + 1, None);
        }

        let file_desc = FileDescriptor {
            fd,
            flags,
            offset: 0,
            file,
            inode_no,
        };

        self.fds[fd as usize] = Some(file_desc);
        self.next_fd = fd + 1;
        let fds_len = self.fds.len();
        while (self.next_fd as usize) < fds_len && self.fds[self.next_fd as usize].is_some() {
            self.next_fd += 1;
        }

        vfslog!(
            "[fd] alloc_fd: Allocated fd = {} with inode = {}",
            fd,
            inode_no
        );
        fd
    }

    /// Free file descriptor
    pub fn free_fd(&mut self, fd: c_int) -> Result<(), Error> {
        // free std fd is not allowed
        if fd < FIRST_FD {
            vfslog!("[fd] free_fd: Cannot free standard file descriptor: {}", fd);
            return Err(code::EBADF);
        }

        if fd as usize >= self.fds.len() {
            vfslog!("[fd] free_fd: Invalid fd: {}", fd);
            return Err(code::EBADF);
        }

        if self.fds[fd as usize].is_none() {
            vfslog!("[fd] free_fd: Fd {} not in use", fd);
            return Err(code::EBADF);
        }

        self.fds[fd as usize] = None;
        if fd < self.next_fd {
            self.next_fd = fd;
        }

        vfslog!("[fd] free_fd: Freed fd = {}", fd);
        Ok(())
    }

    /// Get file descriptor
    pub fn get_fd(&self, fd: c_int) -> Option<&FileDescriptor> {
        if fd < 0 || fd as usize >= self.fds.len() {
            vfslog!("[fd] get_fd: Invalid fd: {}", fd);
            return None;
        }

        let result = self.fds[fd as usize].as_ref();
        if result.is_none() {
            vfslog!("[fd] get_fd: Fd {} not found", fd);
        }
        result
    }

    /// Get mutable file descriptor
    pub fn get_fd_mut(&mut self, fd: c_int) -> Option<&mut FileDescriptor> {
        if fd < 0 || fd as usize >= self.fds.len() {
            vfslog!("[fd] get_fd_mut: Invalid fd: {}", fd);
            return None;
        }

        let result = self.fds[fd as usize].as_mut();
        if result.is_none() {
            vfslog!("[fd] get_fd_mut: Fd {} not found", fd);
        }
        result
    }

    /// Check if file descriptor is valid
    pub fn is_valid_fd(&self, fd: c_int) -> bool {
        fd >= 0 && (fd as usize) < self.fds.len() && self.fds[fd as usize].is_some()
    }

    /// Get current number of allocated file descriptors
    pub fn count(&self) -> usize {
        self.fds.iter().filter(|fd| fd.is_some()).count()
    }
}

// Global file descriptor manager instance
static FD_MANAGER: Once<Arc<SpinLock<FdManager>>> = Once::new();

/// Get file descriptor manager instance
pub(crate) fn get_fd_manager() -> &'static Arc<SpinLock<FdManager>> {
    FD_MANAGER.call_once(|| Arc::new(SpinLock::new(FdManager::new())))
}
