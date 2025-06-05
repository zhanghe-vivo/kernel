//! vfs_fd.rs  
#![allow(dead_code)]

use crate::{
    error::{code, Error},
    vfs::{vfs_mnt, vfs_node::InodeNo, vfs_traits::*},
};
use alloc::{sync::Arc, vec::Vec};
use core::ffi::c_int;
use log::{info, warn};
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
    pub open_flags: c_int,
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
            warn!("[fd] init_stdio: Failed to find filesystem for /dev/console");
            return Err(code::ENOENT);
        };
        let stdin_inode = fs.open("console", libc::O_RDONLY)?;
        let stdout_inode = fs.open("console", libc::O_WRONLY)?;
        let stderr_inode = fs.open("console", libc::O_WRONLY)?;
        let file_ops = as_file_ops(fs);

        self.fds[STDIN_FILENO as usize] = Some(FileDescriptor {
            fd: STDIN_FILENO,
            open_flags: libc::O_RDONLY,
            offset: 0,
            file: file_ops.clone(),
            inode_no: stdin_inode,
        });

        self.fds[STDOUT_FILENO as usize] = Some(FileDescriptor {
            fd: STDOUT_FILENO,
            open_flags: libc::O_WRONLY,
            offset: 0,
            file: file_ops.clone(),
            inode_no: stdout_inode,
        });

        self.fds[STDERR_FILENO as usize] = Some(FileDescriptor {
            fd: STDERR_FILENO,
            open_flags: libc::O_WRONLY,
            offset: 0,
            file: file_ops.clone(),
            inode_no: stderr_inode,
        });

        Ok(())
    }

    /// Allocate new file descriptor
    pub fn alloc_fd(
        &mut self,
        open_flags: c_int,
        file: Arc<dyn FileOperationTrait>,
        inode_no: InodeNo,
    ) -> c_int {
        let fd = self.next_fd;

        if (fd as usize) >= self.fds.len() {
            self.fds.resize(fd as usize + 1, None);
        }

        let file_desc = FileDescriptor {
            fd,
            open_flags,
            offset: 0,
            file,
            inode_no,
        };

        self.fds[fd as usize] = Some(file_desc);
        self.update_next_fd(fd);

        info!(
            "[fd] alloc_fd: Allocated fd = {} with inode = {}",
            fd, inode_no
        );
        fd
    }

    /// Duplicate file descriptor
    pub fn dup_fd(&mut self, fd: c_int, minfd: c_int, close_on_exec: bool) -> Result<c_int, Error> {
        let Some(fd_entry) = self.get_fd(fd) else {
            return Err(code::EBADF);
        };

        // Clone fd_entry early to avoid borrow conflicts
        let fd_entry = fd_entry.clone();

        let mut new_fd = minfd as usize;
        if minfd < FIRST_FD {
            new_fd = FIRST_FD as usize;
        }
        while new_fd < self.fds.len() && self.fds[new_fd].is_some() {
            new_fd += 1;
        }
        if new_fd >= self.fds.len() {
            // add capacity by std Vec, so we just +1
            self.fds.resize(new_fd + 1, None);
        }

        self.fds[new_fd] = Some(FileDescriptor {
            fd: new_fd as c_int,
            open_flags: fd_entry.open_flags | if close_on_exec { libc::O_CLOEXEC } else { 0 },
            offset: fd_entry.offset,
            file: fd_entry.file.clone(),
            inode_no: fd_entry.inode_no,
        });
        self.update_next_fd(new_fd as c_int);
        Ok(new_fd as c_int)
    }

    /// Free file descriptor
    pub fn free_fd(&mut self, fd: c_int) -> Result<(), Error> {
        // free std fd is not allowed
        if fd < FIRST_FD {
            warn!("[fd] free_fd: Cannot free standard file descriptor: {}", fd);
            return Err(code::EBADF);
        }

        if fd as usize >= self.fds.len() {
            warn!("[fd] free_fd: Invalid fd: {}", fd);
            return Err(code::EBADF);
        }

        if self.fds[fd as usize].is_none() {
            warn!("[fd] free_fd: Fd {} not in use", fd);
            return Err(code::EBADF);
        }

        self.fds[fd as usize] = None;
        if fd < self.next_fd {
            self.next_fd = fd;
        }

        info!("[fd] free_fd: Freed fd = {}", fd);
        Ok(())
    }

    /// Get file descriptor
    pub fn get_fd(&self, fd: c_int) -> Option<&FileDescriptor> {
        if fd < 0 || fd as usize >= self.fds.len() {
            warn!("[fd] get_fd: Invalid fd: {}", fd);
            return None;
        }

        let result = self.fds[fd as usize].as_ref();
        if result.is_none() {
            warn!("[fd] get_fd: Fd {} not found", fd);
        }
        result
    }

    /// Get mutable file descriptor
    pub fn get_fd_mut(&mut self, fd: c_int) -> Option<&mut FileDescriptor> {
        if fd < 0 || fd as usize >= self.fds.len() {
            warn!("[fd] get_fd_mut: Invalid fd: {}", fd);
            return None;
        }

        let result = self.fds[fd as usize].as_mut();
        if result.is_none() {
            warn!("[fd] get_fd_mut: Fd {} not found", fd);
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

    fn update_next_fd(&mut self, new_fd: c_int) {
        // Ensure new_fd is valid
        assert!(new_fd >= 0, "Invalid file descriptor");

        let new_fd = new_fd as usize;
        let fds_len = self.fds.len();

        // First try: find free fd from new_fd+1 to end
        let next_fd = (new_fd + 1..fds_len)
            .find(|&fd| self.fds[fd].is_none())
            .unwrap_or_else(|| {
                // Second try: find free fd from start to new_fd
                (FIRST_FD as usize..new_fd)
                    .find(|&fd| self.fds[fd].is_none())
                    .unwrap_or(fds_len) // If still not found, use fds_len
            });

        // Extend array if needed
        if next_fd >= fds_len {
            self.fds.resize(next_fd + 1, None);
        }

        self.next_fd = next_fd as c_int;
    }
}

// Global file descriptor manager instance
static FD_MANAGER: Once<Arc<SpinLock<FdManager>>> = Once::new();

/// Get file descriptor manager instance
pub(crate) fn get_fd_manager() -> &'static Arc<SpinLock<FdManager>> {
    FD_MANAGER.call_once(|| Arc::new(SpinLock::new(FdManager::new())))
}
