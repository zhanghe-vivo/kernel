use crate::{
    error::{code, Error},
    vfs::{
        vfs_dirent::*, vfs_fd::*, vfs_manager::*, vfs_mnt, vfs_mode::*, vfs_node::*, vfs_path::*,
        vfs_traits::*,
    },
};
use alloc::{
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use core::{
    ffi::{c_char, c_int, c_void},
    slice,
};
use libc::{O_ACCMODE, O_CREAT, O_DIRECTORY, O_EXCL, O_RDONLY, O_RDWR, O_TRUNC, O_WRONLY, S_IFDIR};
use log::{info, warn};
use spin::RwLock as SpinRwLock;

fn access_to_mode(access_mode: AccessMode, base_mode: mode_t) -> mode_t {
    let mut mode = base_mode;
    mode &= 0o700;

    let additional_bits = match access_mode {
        AccessMode::O_RDONLY => 0o044,
        AccessMode::O_WRONLY => 0o022,
        AccessMode::O_RDWR => 0o066,
    };

    mode | additional_bits
}

/// Mount a filesystem
pub fn mount(
    source: Option<&str>,
    target: &str,
    fs_type: &str,
    flags: u64,
    data: Option<&[u8]>,
) -> i32 {
    info!(
        "[posix] Mounting {} at {} (type: {})",
        source.unwrap_or("none"),
        target,
        fs_type
    );

    let mount_manager = vfs_mnt::get_mount_manager();

    // Normalize target path
    let target_path = match normalize_path(target) {
        Some(path) => path,
        None => {
            warn!("Invalid target path: {}", target);
            return code::EINVAL.to_errno();
        }
    };

    // Check if target path is already mounted
    if let Some(mount_point) = mount_manager.find_mount(&target_path) {
        if mount_point.path == target_path {
            warn!("[posix] Target path already mounted: {}", target_path);
            return code::EEXIST.to_errno();
        }
    }

    let vfs_manager = get_vfs_manager();
    let fs = match vfs_manager.get_fs(fs_type) {
        Some(fs) => fs,
        None => {
            warn!("Filesystem type not found: {}", fs_type);
            return code::EAGAIN.to_errno();
        }
    };

    // Create mount point
    let mount_point = vfs_mnt::MountPoint {
        path: target_path.clone(),
        fs_type: fs_type.to_string(),
        device: source.map(String::from),
        flags,
        fs: fs.clone(),
    };

    info!(
        "[posix] Created mount point: {} (type: {})",
        mount_point.path, mount_point.fs_type
    );

    match fs.mount(source.unwrap_or(""), &target_path, flags, data) {
        Err(err) => {
            warn!("[posix] Mount failed: {}", err);
            return err.to_errno();
        }
        Ok(_) => {}
    }

    info!(
        "[posix] Successfully mounted {} at {}",
        fs_type, target_path
    );
    mount_manager
        .add_mount(mount_point)
        .map_or_else(|err| err.to_errno(), |_| code::EOK.to_errno())
}

/// Unmount a filesystem
pub fn unmount(target: &str) -> i32 {
    let mount_manager = vfs_mnt::get_mount_manager();

    // Normalize path
    let target_path = match normalize_path(target) {
        Some(path) => path,
        None => {
            warn!("Invalid target path: {}", target);
            return code::EINVAL.to_errno();
        }
    };

    // find mount point
    let mount_point = match mount_manager.find_mount(&target_path) {
        Some(mp) => mp,
        None => {
            warn!("Mount point not found: {}", target_path);
            return code::EINVAL.to_errno();
        }
    };

    let _ = mount_point.fs.unmount(&target_path).map_err(|err| {
        return err.to_errno();
    });
    mount_manager
        .remove_mount(&target_path)
        .map_or_else(|err| err.to_errno(), |_| code::EOK.to_errno())
}

/// Open a file with optional mode parameter
pub fn open(file_path: &str, flags: c_int, _mode: mode_t) -> i32 {
    info!(
        "[posix] open: path = {}, flags = {}",
        file_path,
        flags_to_string(flags)
    );

    let access_mode = AccessMode::from(flags & O_ACCMODE);

    // Check if it's a device file path
    if file_path.starts_with("/dev/") {
        info!("[posix] Opening device file: {}", file_path);

        // Get device filesystem and relative path
        let (fs, relative_path) = match vfs_mnt::find_filesystem(file_path) {
            Some(x) => x,
            None => {
                warn!("Device filesystem not found for path: {}", file_path);
                return code::ENOENT.to_errno();
            }
        };

        // For device files, directly call filesystem's open method
        // devfs will check if the device exists
        // Call filesystem's open method to get inode number
        let inode_no = match fs.open(&relative_path, flags) {
            Ok(fd) => fd as InodeNo,
            Err(err) => return err.to_errno(),
        };

        let mut fd_manager = get_fd_manager().lock();
        let file_ops = as_file_ops(fs);
        return fd_manager.alloc_fd(flags, file_ops, inode_no);
    }

    // Get DNode cache
    let dnode_cache = match get_dnode_cache() {
        Some(cache) => cache,
        None => {
            warn!("Failed to get DNode cache");
            return code::EAGAIN.to_errno();
        }
    };

    let file_found_result: Option<(Arc<dyn VfsOperations>, String)> =
        // TODO: Is the dnode cache needed?
        if let Some(dnode) = dnode_cache.lookup(file_path) {
            // Found existing dnode cache
            // Get filesystem operations object
            let fs = dnode.get_inode().fs_ops.clone();
            // Get relative path
            let relative_path = match vfs_mnt::find_filesystem(file_path) {
                Some((_, path)) => path,
                None => {
                    return code::ENOENT.to_errno();
                }
            };
            Some((fs, relative_path))
        } else {
            // lookup through the fs api 
            if let Some((fs, relative_path)) = vfs_mnt::find_filesystem(file_path) {
                if let Ok(_) = fs.lookup_path(file_path) {
                    // Found existing inode
                    Some((fs, relative_path))
                } else {
                    None
                }
            } else {
                return code::ENOENT.to_errno();
            }
        };
    if let Some((fs, relative_path)) = file_found_result {
        // File found
        // Check O_EXCL flag
        if (flags & O_CREAT != 0) && (flags & O_EXCL != 0) {
            warn!("File exists and O_EXCL specified");
            return code::EEXIST.to_errno();
        }

        // If O_TRUNC is specified, truncate the file
        if flags & O_TRUNC != 0 {
            if access_mode == AccessMode::O_RDONLY {
                return code::EINVAL.to_errno();
            }
            // TODO: Implement file truncation
            // fs.truncate(&relative_path, 0)?;
        }

        // Open existing file
        let inode_no = match fs.open(&relative_path, flags) {
            Ok(fd) => fd as InodeNo,
            Err(err) => {
                warn!("Failed to open existing file: {}", err);
                return err.to_errno();
            }
        };

        // Allocate file descriptor
        let mut fd_manager = get_fd_manager().lock();
        let file_ops = as_file_ops(fs);
        return fd_manager.alloc_fd(flags, file_ops, inode_no);
    } else {
        // File not found
        // If not found in cache and fs, and O_CREAT not specified, return error
        if flags & O_CREAT == 0 {
            warn!("File not found and O_CREAT not specified: {}", file_path);
            return code::ENOENT.to_errno();
        }

        // Get parent directory path and filename
        let (parent_path, file_name) = match split_path(file_path) {
            Some(x) => x,
            None => {
                warn!("Invalid path: {}", file_path);
                return code::EINVAL.to_errno();
            }
        };

        // Get filesystem and relative path
        let (fs, relative_path) = match vfs_mnt::find_filesystem(file_path) {
            Some(x) => x,
            None => {
                warn!("Filesystem not found for path: {}", file_path);
                return code::ENOENT.to_errno();
            }
        };

        // Create new inode
        // we do not have execute permission for now
        let mode: u32 = access_to_mode(access_mode, 0o644);
        let inode_attr = match fs.create_inode(&relative_path, mode) {
            Ok(attr) => attr,
            Err(err) => {
                warn!("Failed to create inode: {}", err);
                return err.to_errno();
            }
        };

        // Open newly created file
        let inode_no = match fs.open(&relative_path, flags) {
            Ok(fd) => fd as InodeNo,
            Err(err) => {
                return err.to_errno();
            }
        };

        // If the parent DNode is found, add new child DNode
        match dnode_cache.lookup(parent_path) {
            Some(parent_dnode) => {
                // Create new Inode and DNode
                let new_inode = Arc::new(Inode::new(inode_attr, fs.clone()));
                let new_dnode = Arc::new(DNode::new(
                    file_name.to_string(),
                    new_inode,
                    Some(Arc::clone(&parent_dnode)),
                ));

                // Add new DNode to parent directory's children
                parent_dnode.add_child(file_name.to_string(), Arc::clone(&new_dnode));
                info!(
                    "[posix] Added new DNode '{}' to parent directory '{}'",
                    file_name,
                    parent_dnode.get_full_path()
                );

                // Add new DNode to cache
                dnode_cache.insert(Arc::clone(&new_dnode));
            }
            None => {
                warn!("Parent dnode cache not found: path {}", parent_path);
            }
        };

        let mut fd_manager = get_fd_manager().lock();
        let file_ops = as_file_ops(fs);
        fd_manager.alloc_fd(flags, file_ops, inode_no)
    }
}

/// Close a file descriptor  
pub fn close(fd: c_int) -> c_int {
    info!("[posix] close: fd = {}", fd);

    let mut fd_manager = get_fd_manager().lock();
    let fd_entry = match fd_manager.get_fd_mut(fd) {
        Some(entry) => entry,
        None => return code::EBADF.to_errno(),
    };

    if let Err(err) = fd_entry.file.close(fd_entry.inode_no) {
        warn!("[posix] close: Failed to close file: {}", err);
    }

    // Always free the fd
    fd_manager
        .free_fd(fd)
        .map_or_else(|e| e.to_errno(), |_| code::EOK.to_errno())
}

pub fn unlink(file_path: &str) -> i32 {
    info!("[posix] unlink: path = {}", file_path);

    // Check path validity
    if !is_valid_path(file_path) {
        warn!("Invalid path: {}", file_path);
        return code::EINVAL.to_errno();
    }

    if file_path.starts_with("/dev/") {
        warn!(
            "Unlink operation on device file is not allowed: {}",
            file_path
        );
        return code::EPERM.to_errno();
    }

    // Get DNode cache
    let dnode_cache = match get_dnode_cache() {
        Some(cache) => cache,
        None => {
            warn!("Failed to get DNode cache");
            return code::EAGAIN.to_errno();
        }
    };

    // Get parent directory path and filename
    let (parent_path, file_name) = match split_path(file_path) {
        Some(x) => x,
        None => {
            warn!("Invalid path: {}", file_path);
            return code::EINVAL.to_errno();
        }
    };

    let parent_dnode: Arc<DNode>;
    // Look up directory node to be deleted
    let dnode = match dnode_cache.lookup(file_path) {
        Some(dnode) => {
            // Check if it's a directory
            if dnode.get_inode().is_dir() {
                warn!("is a directory: {}", file_path);
                return code::EISDIR.to_errno();
            }
            // Check if it's root directory
            parent_dnode = match dnode.get_parent() {
                Some(dnode) => dnode,
                None => {
                    warn!("Cannot remove root directory");
                    return code::EBUSY.to_errno();
                }
            };
            dnode
        }
        None => {
            // Look up parent directory's DNode
            info!("[posix] Looking up parent directory: {}", parent_path);
            parent_dnode = match dnode_cache.lookup(parent_path) {
                Some(dnode) => dnode,
                None => {
                    warn!("Parent directory not found: {}", parent_path);
                    return code::ENOENT.to_errno();
                }
            };
            let dnode = parent_dnode.find_child(&file_name.to_string()).unwrap();
            if dnode.get_inode().is_dir() {
                warn!("is a directory: {}", file_path);
                return code::EISDIR.to_errno();
            }

            dnode
        }
    };

    let fs = dnode.get_inode().fs_ops.clone();
    // FIXME: get relative path from dnode
    let (_, relative_path) = match vfs_mnt::find_filesystem(file_path) {
        Some(x) => x,
        None => {
            warn!("Filesystem not found for path: {}", file_path);
            return code::ENOENT.to_errno();
        }
    };

    // Call filesystem's remove inode operation
    match fs.remove_inode(&relative_path) {
        Ok(()) => {
            // Remove directory from parent
            parent_dnode.remove_child(&file_name.to_string());

            // Remove directory from DNode cache
            dnode_cache.remove(file_path);

            info!("[posix] Successfully removed directory: {}", file_path);
            code::EOK.to_errno()
        }
        Err(err) => {
            warn!("Failed to remove directory: {}", err);
            err.to_errno()
        }
    }
}

/// Read from a file  
pub fn read(fd: c_int, buf: *mut c_void, len: usize) -> isize {
    if buf.is_null() {
        return code::EINVAL.to_errno() as isize;
    }

    let mut fd_manager = get_fd_manager().lock();
    let fd_entry = match fd_manager.get_fd_mut(fd) {
        Some(entry) => entry,
        None => return code::EBADF.to_errno() as isize,
    };
    if (fd_entry.open_flags & O_ACCMODE) == O_WRONLY {
        warn!("fd {} is write only", fd);
        return code::EBADF.to_errno() as isize;
    }
    let buffer = unsafe { slice::from_raw_parts_mut(buf as *mut u8, len) };

    // Pass offset directly, no extra seek call needed
    match fd_entry
        .file
        .read(fd_entry.inode_no, buffer, &mut fd_entry.offset)
    {
        Ok(n) => n as isize,
        Err(err) => err.to_errno() as isize,
    }
}

/// Write to a file  
pub fn write(fd: i32, buf: &[u8], count: usize) -> isize {
    info!("write: fd = {}, count = {}", fd, count);

    let mut fd_manager = get_fd_manager().lock();

    let fd_entry = match fd_manager.get_fd_mut(fd) {
        Some(entry) => entry,
        None => return code::EBADF.to_errno() as isize,
    };

    if (fd_entry.open_flags & O_ACCMODE) == O_RDONLY {
        warn!("fd {} is read only", fd);
        return code::EBADF.to_errno() as isize;
    }

    // Pass offset directly, no extra seek call needed
    match fd_entry
        .file
        .write(fd_entry.inode_no, buf, &mut fd_entry.offset)
    {
        Ok(n) => n as isize,
        Err(err) => err.to_errno() as isize,
    }
}

/// Seek in a file
pub fn lseek(fd: c_int, offset: i64, whence: c_int) -> i64 {
    info!(
        "lseek: fd = {}, offset = {}, whence = {}",
        fd, offset, whence
    );

    let mut fd_manager = get_fd_manager().lock();

    let fd_entry = match fd_manager.get_fd_mut(fd) {
        Some(entry) => entry,
        None => return code::EBADF.to_errno() as i64,
    };

    // Calculate new offset
    let new_offset = match whence {
        SEEK_SET => offset,
        SEEK_CUR => fd_entry.offset as i64 + offset,
        SEEK_END => {
            let size = fd_entry
                .file
                .size(fd_entry.inode_no)
                .map_or_else(|err| return err.to_errno() as i64, |size| size as i64);
            size + offset
        }
        _ => return code::EINVAL.to_errno() as i64,
    };

    // Check if offset is valid
    if new_offset < 0 {
        return code::EINVAL.to_errno() as i64;
    }

    // Update file offset
    fd_entry.offset = new_offset as usize;

    // Execute seek operation with new offset and whence
    match fd_entry
        .file
        .seek(fd_entry.inode_no, new_offset as usize, whence)
    {
        Ok(_) => new_offset,
        Err(err) => err.to_errno() as i64,
    }
}

/// Create a directory
pub fn mkdir(path: &str, mode: mode_t) -> i32 {
    info!("[posix] mkdir: path = {}, mode = {:o}", path, mode);

    // Check path validity
    if !is_valid_path(path) {
        warn!("Invalid path: {}", path);
        return code::EINVAL.to_errno();
    }

    // Get parent directory path and directory name
    let (parent_path, dir_name) = match split_path(path) {
        Some(x) => x,
        None => {
            warn!("Invalid path: {}", path);
            return code::EINVAL.to_errno();
        }
    };

    // Get filesystem
    let (fs, relative_path) = match vfs_mnt::find_filesystem(path) {
        Some(x) => x,
        None => {
            warn!("Filesystem not found for path: {}", path);
            return code::ENOENT.to_errno();
        }
    };

    // Get DNode cache
    let dnode_cache = match get_dnode_cache() {
        Some(cache) => cache,
        None => {
            warn!("Failed to get DNode cache");
            return code::EAGAIN.to_errno();
        }
    };

    // Look up parent directory's DNode
    let parent_dnode = match dnode_cache.lookup(parent_path) {
        Some(dnode) => dnode,
        None => {
            warn!("Parent directory not found: {}", parent_path);
            return code::ENOENT.to_errno();
        }
    };

    // Check if parent is a directory
    if !parent_dnode.get_inode().attr.read().is_dir() {
        warn!("Parent is not a directory: {}", parent_path);
        return code::ENOTDIR.to_errno();
    }

    // Check if directory already exists
    if dir_name.is_empty() || parent_dnode.find_child(&dir_name).is_some() {
        warn!("Directory already exists: {}", path);
        return code::EEXIST.to_errno();
    }

    // Create directory
    let mode = mode | S_IFDIR; // Add directory flag
    match fs.create_inode(&relative_path, mode) {
        Ok(attr) => {
            // Create new Inode and DNode
            let new_inode = Arc::new(Inode::new(attr, fs.clone()));
            let new_dnode = Arc::new(DNode::new(
                dir_name.to_string(),
                new_inode,
                Some(Arc::clone(&parent_dnode)),
            ));

            // Add new DNode to parent directory's children
            parent_dnode.add_child(dir_name.to_string(), Arc::clone(&new_dnode));

            // Add new DNode to cache
            dnode_cache.insert(Arc::clone(&new_dnode));

            info!("[posix] Successfully created directory: {}", path);
            code::EOK.to_errno()
        }
        Err(err) => {
            warn!("Failed to create directory: {}", err);
            err.to_errno()
        }
    }
}

/// Remove a directory
pub fn rmdir(path: &str) -> i32 {
    info!("[posix] rmdir: path = {}", path);

    // Check path validity
    if !is_valid_path(path) {
        warn!("Invalid path: {}", path);
        return code::EINVAL.to_errno();
    }

    // Get DNode cache
    let dnode_cache = match get_dnode_cache() {
        Some(cache) => cache,
        None => {
            warn!("Failed to get DNode cache");
            return code::EAGAIN.to_errno();
        }
    };

    // Get parent directory path and filename
    let (parent_path, dir_name) = match split_path(path) {
        Some(x) => x,
        None => {
            warn!("Invalid path: {}", path);
            return code::EINVAL.to_errno();
        }
    };

    let mut parent_dnode: Arc<DNode>;
    // Look up directory node to be deleted
    let dnode = match dnode_cache.lookup(path) {
        Some(dnode) => {
            // Check if it's a directory
            if !dnode.get_inode().is_dir() {
                warn!("Not a directory: {}", path);
                return code::ENOTDIR.to_errno();
            }
            // Check if it's root directory
            parent_dnode = match dnode.get_parent() {
                Some(dnode) => dnode,
                None => {
                    warn!("Cannot remove root directory");
                    return code::EBUSY.to_errno();
                }
            };
            dnode
        }
        None => {
            // Look up parent directory's DNode
            info!("[posix] Looking up parent directory: {}", parent_path);
            parent_dnode = match dnode_cache.lookup(parent_path) {
                Some(dnode) => dnode,
                None => {
                    warn!("Parent directory not found: {}", parent_path);
                    return code::ENOENT.to_errno();
                }
            };
            let dnode = match parent_dnode.find_child(&dir_name.to_string()) {
                Some(dnode) => dnode,
                None => {
                    warn!("Directory not found: {}", path);
                    return code::ENOENT.to_errno();
                }
            };
            if !dnode.get_inode().is_dir() {
                warn!("Not a directory: {}", path);
                return code::ENOTDIR.to_errno();
            }

            dnode
        }
    };

    let fs = dnode.get_inode().fs_ops.clone();
    // FIXME: get relative path from dnode
    let (_, relative_path) = match vfs_mnt::find_filesystem(path) {
        Some(x) => x,
        None => {
            warn!("Filesystem not found for path: {}", path);
            return code::ENOENT.to_errno();
        }
    };

    // Call filesystem's remove inode operation
    match fs.remove_inode(&relative_path) {
        Ok(()) => {
            // Remove directory from parent
            parent_dnode.remove_child(&dir_name.to_string());

            // Remove directory from DNode cache
            dnode_cache.remove(path);

            info!("[posix] Successfully removed directory: {}", path);
            code::EOK.to_errno()
        }
        Err(err) => {
            warn!("Failed to remove directory: {}", err);
            err.to_errno()
        }
    }
}

/// Open directory
pub fn opendir(path: &str) -> Result<Arc<Dir>, Error> {
    info!("[posix] opendir: path = {}", path);

    // Get DNode cache
    let dnode_cache = get_dnode_cache().ok_or(code::ENOSYS)?;

    // Look up directory node from cache
    if let Some(dnode) = dnode_cache.lookup(path) {
        if !dnode.get_inode().attr.read().is_dir() {
            warn!("[posix] Not a directory: {}", path);
            return Err(code::ENOTDIR);
        }
    };

    let (fs, relative_path) = match vfs_mnt::find_filesystem(path) {
        Some(x) => x,
        None => {
            warn!("[posix] Filesystem not found for path: {}", path);
            return Err(code::ENOENT);
        }
    };

    let inode_no = fs.open(&relative_path, O_RDONLY | O_DIRECTORY)?;
    let mut fd_manager = get_fd_manager().lock();
    let file_ops = as_file_ops(fs);
    let fd = fd_manager.alloc_fd(O_RDONLY | O_DIRECTORY, file_ops, inode_no);

    if fd < 0 {
        warn!("[posix] Failed to allocate fd for directory");
        return Err(code::ENOMEM);
    }

    Ok(Arc::new(Dir {
        fd,
        state: SpinRwLock::new(DirState::default()),
    }))
}

/// Read directory entry
pub fn readdir(dir: &Arc<Dir>) -> Result<Dirent, Error> {
    info!(
        "[posix] readdir: fd = {}, current offset = {}",
        dir.fd,
        dir.state.read().offset
    );

    // Get file descriptor manager and file descriptor
    let fd_manager = get_fd_manager().lock();
    let fd_entry = fd_manager.get_fd(dir.fd).ok_or(code::EBADF)?;

    // Check if it's a directory
    if fd_entry.open_flags & O_DIRECTORY == 0 {
        warn!("[posix] readdir: Not a directory fd: {}", dir.fd);
        return Err(code::ENOTDIR);
    }

    let mut state = dir.state.write();
    // Prepare to receive single directory entry
    let mut dirents = Vec::with_capacity(1);
    // Read single directory entry
    match fd_entry
        .file
        .getdents(fd_entry.inode_no, state.offset, &mut dirents, 1)
    {
        Ok(_n) => {
            let dirent = dirents.remove(0);
            // Update offset
            state.offset += 1;
            info!("[posix] readdir: Returning entry: {}", dirent.name_as_str());
            Ok(dirent)
        }
        Err(err) => Err(err),
    }
}

/// Close directory
pub fn closedir(dir: Arc<Dir>) -> i32 {
    info!("[posix] closedir: fd = {}", dir.fd);

    let mut fd_manager = get_fd_manager().lock();

    // Check if file descriptor is valid
    let fd_entry = match fd_manager.get_fd(dir.fd) {
        Some(entry) => entry,
        None => return code::EBADF.to_errno(),
    };

    // Check if it's a directory
    if fd_entry.open_flags & O_DIRECTORY == 0 {
        warn!("Not a directory fd: {}", dir.fd);
        return code::ENOTDIR.to_errno();
    }

    fd_manager
        .free_fd(dir.fd)
        .map_or_else(|e| e.to_errno(), |_| code::EOK.to_errno())
}

pub fn fcntl(fd: i32, cmd: c_int, args: usize) -> c_int {
    info!("fcntl: fd = {}, cmd = {}, args = {}", fd, cmd, args);
    const FD_CLOEXEC: c_int = 1;

    match cmd {
        libc::F_DUPFD => {
            let mut fd_manager = get_fd_manager().lock();
            let new_fd = match fd_manager.dup_fd(fd, args as c_int, false) {
                Ok(fd) => fd,
                Err(err) => return err.to_errno(),
            };
            new_fd as c_int
        }
        libc::F_DUPFD_CLOEXEC => {
            let mut fd_manager = get_fd_manager().lock();
            let new_fd = match fd_manager.dup_fd(fd, args as c_int, true) {
                Ok(fd) => fd,
                Err(err) => return err.to_errno(),
            };
            new_fd as c_int
        }
        libc::F_GETFD => {
            let fd_manager = get_fd_manager().lock();
            let fd_entry = match fd_manager.get_fd(fd) {
                Some(entry) => entry,
                None => return code::EBADF.to_errno(),
            };
            if fd_entry.open_flags & libc::O_CLOEXEC != 0 {
                FD_CLOEXEC
            } else {
                0
            }
        }
        libc::F_SETFD => {
            let flags = args as c_int;
            if flags & !FD_CLOEXEC != 0 {
                return code::ENOSYS.to_errno();
            }

            let is_cloexec = (args as c_int) & FD_CLOEXEC != 0;

            let mut fd_manager = get_fd_manager().lock();
            let fd_entry = match fd_manager.get_fd_mut(fd) {
                Some(entry) => entry,
                None => return code::EBADF.to_errno(),
            };
            if is_cloexec {
                fd_entry.open_flags |= libc::O_CLOEXEC;
            } else {
                fd_entry.open_flags &= !libc::O_CLOEXEC;
            }
            0
        }
        libc::F_GETFL => {
            let fd_manager = get_fd_manager().lock();
            let fd_entry = match fd_manager.get_fd(fd) {
                Some(entry) => entry,
                None => return code::EBADF.to_errno(),
            };
            fd_entry.open_flags as c_int
        }
        libc::F_SETFL => {
            // this operation can change only O_NONBLOCK for now
            let mut fd_manager = get_fd_manager().lock();
            let fd_entry = match fd_manager.get_fd_mut(fd) {
                Some(entry) => entry,
                None => return code::EBADF.to_errno(),
            };

            let oflags = args as c_int;
            if oflags & libc::O_NONBLOCK == 0 {
                fd_entry.open_flags &= !libc::O_NONBLOCK;
            } else {
                fd_entry.open_flags |= libc::O_NONBLOCK;
            }
            0
        }

        _ => return code::ENOSYS.to_errno(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bluekernel_test_macro::test;

    // Mock data for testing
    const TEST_PATH: &str = "/test/file.txt";
    const TEST_DIR: &str = "/test";
    const TEST_SUB_DIR: &str = "/test/subdir";
    const TEST_CONTENT: &[u8] = b"Hello, World!";

    #[test]
    fn test_open_invalid_path() {
        // Test with null pointer
        let result = open("", libc::O_RDONLY, 0);
        assert_eq!(result, code::ENOENT.to_errno());
    }

    #[test]
    fn test_open_create_file() {
        let result = mkdir(TEST_DIR, 0o755);
        assert_eq!(result, code::EOK.to_errno());

        let fd = open(TEST_PATH, libc::O_CREAT | libc::O_WRONLY, 0o644);
        assert!(fd > 0);

        let result = close(fd);
        assert_eq!(result, code::EOK.to_errno());

        let fd = open(TEST_PATH, libc::O_WRONLY, 0);
        assert!(fd > 0);

        let result = close(fd);
        assert_eq!(result, code::EOK.to_errno());

        let result = unlink(TEST_PATH);
        assert_eq!(result, code::EOK.to_errno());

        let result = rmdir(TEST_DIR);
        assert_eq!(result, code::EOK.to_errno());
    }

    #[test]
    fn test_close_invalid_fd() {
        // Test closing invalid file descriptor
        let result = close(-1);
        assert_eq!(result, code::EBADF.to_errno());

        let result = close(1000);
        assert_eq!(result, code::EBADF.to_errno());
    }

    #[test]
    fn test_read_invalid_params() {
        // Test with null buffer
        let result = read(0, core::ptr::null_mut(), 100);
        assert_eq!(result, code::EINVAL.to_errno() as isize);

        // Test with invalid fd
        let mut buffer = [0u8; 100];
        let result = read(-1, buffer.as_mut_ptr() as *mut c_void, 100);
        assert_eq!(result, code::EBADF.to_errno() as isize);
    }

    #[test]
    fn test_write_invalid_fd() {
        let result = write(-1, b"test", 4);
        assert_eq!(result, code::EBADF.to_errno() as isize);
    }

    #[test]
    fn test_lseek_invalid_params() {
        // Test with invalid file descriptor
        let result = lseek(-1, 0, SEEK_SET);
        assert_eq!(result, code::EBADF.to_errno() as i64);

        // Test with invalid whence
        let result = lseek(0, 0, 999);
        assert_eq!(result, code::EINVAL.to_errno() as i64);

        // Test with negative offset for SEEK_SET
        let result = lseek(0, -1, SEEK_SET);
        assert_eq!(result, code::EINVAL.to_errno() as i64);
    }

    #[test]
    fn test_mkdir_invalid_path() {
        // Test with empty path
        let result = mkdir("", 0o755);
        assert_eq!(result, code::EINVAL.to_errno());

        // Test with root path
        let result = mkdir("/", 0o755);
        assert_eq!(result, code::EEXIST.to_errno());
    }

    #[test]
    fn test_rmdir_invalid_path() {
        // Test with empty path
        let result = rmdir("");
        assert_eq!(result, code::EINVAL.to_errno());

        // Test with non-existent path
        let result = rmdir(TEST_DIR);
        assert_eq!(result, code::ENOENT.to_errno());
    }

    #[test]
    fn test_dir() {
        assert!(opendir(TEST_DIR).is_err());

        let result = mkdir(TEST_SUB_DIR, 0o755);
        assert_eq!(result, code::ENOENT.to_errno());

        let result = mkdir(TEST_DIR, 0o755);
        assert_eq!(result, code::EOK.to_errno());

        let result = mkdir(TEST_SUB_DIR, 0o755);
        assert_eq!(result, code::EOK.to_errno());

        let dir = match opendir(TEST_SUB_DIR) {
            Ok(dir) => dir,
            Err(err) => {
                assert!(false, "Failed to open directory: {}", err);
                return;
            }
        };

        let dirent = match readdir(&dir) {
            Ok(dirent) => dirent,
            Err(err) => {
                assert!(false, "Failed to read directory: {}", err);
                return;
            }
        };
        assert_eq!(dirent.name_as_str(), ".");

        let result = closedir(dir);
        assert_eq!(result, code::EOK.to_errno());

        let result = rmdir(TEST_DIR);
        assert_eq!(result, code::ENOTEMPTY.to_errno());

        let result = rmdir(TEST_SUB_DIR);
        assert_eq!(result, code::EOK.to_errno());

        let result = rmdir(TEST_DIR);
        assert_eq!(result, code::EOK.to_errno());
    }

    #[test]
    fn test_fcntl_invalid_params() {
        // Test F_GETFD with invalid fd
        let result = fcntl(-1, libc::F_GETFD, 0);
        assert_eq!(result, code::EBADF.to_errno());

        // Test F_SETFD with invalid fd
        let result = fcntl(-1, libc::F_SETFD, 0);
        assert_eq!(result, code::EBADF.to_errno());

        // Test F_SETFD with invalid flags
        let result = fcntl(0, libc::F_SETFD, 256); // flags > u8::MAX
        assert_eq!(result, code::ENOSYS.to_errno());

        // Test unsupported command
        let result = fcntl(0, 999, 0);
        assert_eq!(result, code::ENOSYS.to_errno());
    }

    #[test]
    fn test_fcntl_dupfd() {
        // Test F_DUPFD with invalid source fd
        let result = fcntl(-1, libc::F_DUPFD, 0);
        assert_eq!(result, code::EBADF.to_errno());

        // Test F_DUPFD_CLOEXEC with invalid source fd
        let result = fcntl(-1, libc::F_DUPFD_CLOEXEC, 0);
        assert_eq!(result, code::EBADF.to_errno());
    }

    #[test]
    fn test_mount_invalid_params() {
        // Test with invalid target path
        let result = mount(None, "", "tmpfs", 0, None);
        assert_eq!(result, code::EINVAL.to_errno());

        // Test with already mounted path
        let result = mount(None, "/", "unknownfs", 0, None);
        assert_eq!(result, code::EEXIST.to_errno());

        // Test with unknown filesystem type
        let result = mount(None, TEST_DIR, "unknownfs", 0, None);
        assert_eq!(result, code::EAGAIN.to_errno());
    }
}
