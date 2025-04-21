use crate::{
    error::{code, Error},
    vfs::{
        vfs_dirent::*, vfs_fd::*, vfs_log::*, vfs_manager::*, vfs_mnt, vfs_mode::*, vfs_node::*,
        vfs_path::*, vfs_traits::*,
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
use spin::RwLock as SpinRwLock;

/// Mount a filesystem
pub fn mount(
    source: Option<&str>,
    target: &str,
    fs_type: &str,
    flags: u64,
    data: Option<&[u8]>,
) -> i32 {
    vfslog!(
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
            vfslog!("Invalid target path: {}", target);
            return code::EINVAL.to_errno();
        }
    };

    vfslog!("[posix] Normalized target path: {}", target_path);

    // Check if target path is already mounted
    if mount_manager.find_mount(&target_path).is_some() {
        vfslog!("[posix] Target path already mounted: {}", target_path);
        return code::EEXIST.to_errno();
    }

    let vfs_manager = get_vfs_manager();
    let fs = match vfs_manager.get_fs(fs_type) {
        Some(fs) => fs,
        None => {
            vfslog!("Filesystem type not found: {}", fs_type);
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

    vfslog!(
        "[posix] Created mount point: {} (type: {})",
        mount_point.path,
        mount_point.fs_type
    );

    let _ = fs
        .mount(source.unwrap_or(""), &target_path, flags, data)
        .map_err(|err| {
            vfslog!("Mount failed: {}", err);
            return err.to_errno();
        });

    vfslog!(
        "[posix] Successfully mounted {} at {}",
        fs_type,
        target_path
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
            vfslog!("Invalid target path: {}", target);
            return code::EINVAL.to_errno();
        }
    };

    // find mount point
    let mount_point = match mount_manager.find_mount(&target_path) {
        Some(mp) => mp,
        None => {
            vfslog!("Mount point not found: {}", target_path);
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
pub fn open(file: *const c_char, flags: c_int, _mode: mode_t) -> i32 {
    if file.is_null() {
        return code::EINVAL.to_errno();
    }

    let file_path = match unsafe { core::ffi::CStr::from_ptr(file).to_str() } {
        Ok(s) => s,
        Err(_) => return code::EINVAL.to_errno(),
    };

    vfslog!(
        "[posix] open: path = {}, flags = {}",
        file_path,
        flags_to_string(flags)
    );

    // Validate access mode
    let access_mode = flags & O_ACCMODE;
    if access_mode != O_RDONLY && access_mode != O_WRONLY && access_mode != O_RDWR {
        vfslog!("open: invalid access mode");
        return code::EINVAL.to_errno();
    }

    // Check if it's a device file path
    if file_path.starts_with("/dev/") {
        vfslog!("[posix] Opening device file: {}", file_path);

        // Get device filesystem and relative path
        let (fs, relative_path) = match vfs_mnt::find_filesystem(file_path) {
            Some(x) => x,
            None => {
                vfslog!("Device filesystem not found for path: {}", file_path);
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
            vfslog!("Failed to get DNode cache");
            return code::EAGAIN.to_errno();
        }
    };

    // First look up in cache
    if let Some(dnode) = dnode_cache.lookup(file_path) {
        vfslog!("[posix] Found existing dnode for path: {}", file_path);

        // Check O_EXCL flag
        if (flags & O_CREAT != 0) && (flags & O_EXCL != 0) {
            vfslog!("File exists and O_EXCL specified");
            return code::EEXIST.to_errno();
        }

        // Get filesystem operations object
        let fs = dnode.get_inode().fs_ops.clone();

        // Get relative path
        let relative_path = match vfs_mnt::find_filesystem(file_path) {
            Some((_, path)) => path,
            None => {
                vfslog!("Failed to get relative path for: {}", file_path);
                return code::ENOENT.to_errno();
            }
        };

        // If O_TRUNC is specified, truncate the file
        if flags & O_TRUNC != 0 {
            if access_mode == O_RDONLY {
                return code::EINVAL.to_errno();
            }
            // TODO: Implement file truncation
            // fs.truncate(&relative_path, 0)?;
        }

        // Open existing file
        let inode_no = match fs.open(&relative_path, flags) {
            Ok(fd) => fd as InodeNo,
            Err(err) => {
                vfslog!("Failed to open existing file: {}", err);
                return err.to_errno();
            }
        };

        // Allocate file descriptor
        let mut fd_manager = get_fd_manager().lock();
        let file_ops = as_file_ops(fs);
        return fd_manager.alloc_fd(flags, file_ops, inode_no);
    }

    // If not found in cache and O_CREAT not specified, return error
    if flags & O_CREAT == 0 {
        vfslog!("File not found and O_CREAT not specified: {}", file_path);
        return code::ENOENT.to_errno();
    }

    // Get parent directory path and filename
    let (parent_path, file_name) = match split_path(file_path) {
        Some(x) => x,
        None => {
            vfslog!("Invalid path: {}", file_path);
            return code::EINVAL.to_errno();
        }
    };

    // Look up parent directory's DNode
    vfslog!("[posix] Looking up parent directory: {}", parent_path);
    let parent_dnode = match dnode_cache.lookup(parent_path) {
        Some(dnode) => dnode,
        None => {
            vfslog!("Parent directory not found: {}", parent_path);
            return code::ENOENT.to_errno();
        }
    };

    // Get filesystem and relative path
    let (fs, relative_path) = match vfs_mnt::find_filesystem(file_path) {
        Some(x) => x,
        None => {
            vfslog!("Filesystem not found for path: {}", file_path);
            return code::ENOENT.to_errno();
        }
    };

    // Create new file with default mode
    // Note: We ignore the mode from varargs and use default value
    let mode = 0o644; // Default file permissions
    let inode_attr = match fs.create_inode(&relative_path, mode) {
        Ok(attr) => attr,
        Err(err) => {
            vfslog!("Failed to create inode: {}", err);
            return err.to_errno();
        }
    };

    // Create new Inode and DNode
    let new_inode = Arc::new(Inode::new(inode_attr, fs.clone()));
    let new_dnode = Arc::new(DNode::new(
        file_name.to_string(),
        new_inode,
        Some(Arc::clone(&parent_dnode)),
    ));

    // Add new DNode to parent directory's children
    parent_dnode.add_child(file_name.to_string(), Arc::clone(&new_dnode));
    vfslog!(
        "[posix] Added new DNode '{}' to parent directory '{}'",
        file_name,
        parent_dnode.get_full_path()
    );

    // Add new DNode to cache
    dnode_cache.insert(Arc::clone(&new_dnode));

    // Open newly created file
    let inode_no = match fs.open(&relative_path, flags) {
        Ok(fd) => fd as InodeNo,
        Err(err) => {
            // If it's a newly created file, need to clean up nodes
            if flags & O_CREAT != 0 {
                parent_dnode.remove_child(&file_name.to_string());
                dnode_cache.remove(file_path);
            }
            return err.to_errno();
        }
    };

    let mut fd_manager = get_fd_manager().lock();
    let file_ops = as_file_ops(fs);
    fd_manager.alloc_fd(flags, file_ops, inode_no)
}

/// Close a file descriptor  
pub fn close(fd: c_int) -> c_int {
    vfslog!("[posix] close: fd = {}", fd);

    let mut fd_manager = get_fd_manager().lock();
    let fd_entry = match fd_manager.get_fd_mut(fd) {
        Some(entry) => entry,
        None => return code::EBADF.to_errno(),
    };

    if let Err(err) = fd_entry.file.close(fd_entry.inode_no) {
        vfslog!("[posix] close: Failed to close file: {}", err);
    }

    // Always free the fd
    fd_manager
        .free_fd(fd)
        .map_or_else(|e| e.to_errno(), |_| code::EOK.to_errno())
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
    if (fd_entry.flags & O_ACCMODE) == O_WRONLY {
        vfslog!("fd {} is write only", fd);
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
    vfslog!("write: fd = {}, count = {}", fd, count);

    let mut fd_manager = get_fd_manager().lock();

    let fd_entry = match fd_manager.get_fd_mut(fd) {
        Some(entry) => entry,
        None => return code::EBADF.to_errno() as isize,
    };

    if (fd_entry.flags & O_ACCMODE) == O_RDONLY {
        vfslog!("fd {} is read only", fd);
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
    vfslog!(
        "lseek: fd = {}, offset = {}, whence = {}",
        fd,
        offset,
        whence
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
    vfslog!("[posix] mkdir: path = {}, mode = {:o}", path, mode);

    // Check path validity
    if !is_valid_path(path) {
        vfslog!("Invalid path: {}", path);
        return code::EINVAL.to_errno();
    }

    // Get parent directory path and directory name
    let (parent_path, dir_name) = match split_path(path) {
        Some(x) => x,
        None => {
            vfslog!("Invalid path: {}", path);
            return code::EINVAL.to_errno();
        }
    };

    // Get filesystem
    let (fs, relative_path) = match vfs_mnt::find_filesystem(path) {
        Some(x) => x,
        None => {
            vfslog!("Filesystem not found for path: {}", path);
            return code::ENOENT.to_errno();
        }
    };

    // Get DNode cache
    let dnode_cache = match get_dnode_cache() {
        Some(cache) => cache,
        None => {
            vfslog!("Failed to get DNode cache");
            return code::EAGAIN.to_errno();
        }
    };

    // Look up parent directory's DNode
    let parent_dnode = match dnode_cache.lookup(parent_path) {
        Some(dnode) => dnode,
        None => {
            vfslog!("Parent directory not found: {}", parent_path);
            return code::ENOENT.to_errno();
        }
    };

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

            vfslog!("[posix] Successfully created directory: {}", path);
            code::EOK.to_errno()
        }
        Err(err) => {
            vfslog!("Failed to create directory: {}", err);
            err.to_errno()
        }
    }
}

/// Remove a directory
pub fn rmdir(path: &str) -> i32 {
    vfslog!("[posix] rmdir: path = {}", path);

    // Check path validity
    if !is_valid_path(path) {
        vfslog!("Invalid path: {}", path);
        return code::EINVAL.to_errno();
    }

    // Get DNode cache
    let dnode_cache = match get_dnode_cache() {
        Some(cache) => cache,
        None => {
            vfslog!("Failed to get DNode cache");
            return code::EAGAIN.to_errno();
        }
    };

    // Look up directory node to be deleted
    let dnode = match dnode_cache.lookup(path) {
        Some(dnode) => dnode,
        None => {
            vfslog!("Directory not found: {}", path);
            return code::ENOENT.to_errno();
        }
    };

    // Check if it's a directory
    if !dnode.get_inode().is_dir() {
        vfslog!("Not a directory: {}", path);
        return code::ENOTDIR.to_errno();
    }

    // Check if it's root directory
    if dnode.get_parent().is_none() {
        vfslog!("Cannot remove root directory");
        return code::EBUSY.to_errno();
    }

    // Get filesystem
    let (fs, relative_path) = match vfs_mnt::find_filesystem(path) {
        Some(x) => x,
        None => {
            vfslog!("Filesystem not found for path: {}", path);
            return code::ENOENT.to_errno();
        }
    };

    // Call filesystem's remove inode operation
    match fs.remove_inode(&relative_path) {
        Ok(()) => {
            // Get parent directory path and directory name
            let (_, dir_name) = match split_path(path) {
                Some(x) => x,
                None => {
                    vfslog!("Invalid path: {}", path);
                    return code::EINVAL.to_errno();
                }
            };

            // Remove directory from parent
            if let Some(parent) = dnode.get_parent() {
                parent.remove_child(&dir_name.to_string());
            }

            // Remove directory from DNode cache
            dnode_cache.remove(path);

            vfslog!("[posix] Successfully removed directory: {}", path);
            code::EOK.to_errno()
        }
        Err(err) => {
            vfslog!("Failed to remove directory: {}", err);
            err.to_errno()
        }
    }
}

/// Open directory
pub fn opendir(path: &str) -> Result<Arc<Dir>, Error> {
    vfslog!("[posix] opendir: path = {}", path);

    // Get DNode cache
    let dnode_cache = get_dnode_cache().ok_or(code::ENOSYS)?;

    // Look up directory node from cache
    if let Some(dnode) = dnode_cache.lookup(path) {
        if !dnode.get_inode().attr.read().is_dir() {
            vfslog!("[posix] Not a directory: {}", path);
            return Err(code::ENOTDIR);
        }
    };

    let (fs, relative_path) = match vfs_mnt::find_filesystem(path) {
        Some(x) => x,
        None => {
            vfslog!("[posix] Filesystem not found for path: {}", path);
            return Err(code::ENOENT);
        }
    };

    let inode_no = fs.open(&relative_path, O_RDONLY | O_DIRECTORY)?;
    let mut fd_manager = get_fd_manager().lock();
    let file_ops = as_file_ops(fs);
    let fd = fd_manager.alloc_fd(O_RDONLY | O_DIRECTORY, file_ops, inode_no);

    if fd < 0 {
        vfslog!("[posix] Failed to allocate fd for directory");
        return Err(code::ENOMEM);
    }

    Ok(Arc::new(Dir {
        fd,
        state: SpinRwLock::new(DirState::default()),
    }))
}

/// Read directory entry
pub fn readdir(dir: &Arc<Dir>) -> Result<Dirent, Error> {
    vfslog!(
        "[posix] readdir: fd = {}, current offset = {}",
        dir.fd,
        dir.state.read().offset
    );

    // Get file descriptor manager and file descriptor
    let fd_manager = get_fd_manager().lock();
    let fd_entry = fd_manager.get_fd(dir.fd).ok_or(code::EBADF)?;

    // Check if it's a directory
    if fd_entry.flags & O_DIRECTORY == 0 {
        vfslog!("[posix] readdir: Not a directory fd: {}", dir.fd);
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
            vfslog!("[posix] readdir: Returning entry: {}", dirent.name_as_str());
            Ok(dirent)
        }
        Err(err) => {
            //vfslog!("[posix] readdir: No more entries");
            Err(err)
        }
    }
}

/// Close directory
pub fn closedir(dir: Arc<Dir>) -> i32 {
    vfslog!("[posix] closedir: fd = {}", dir.fd);

    let mut fd_manager = get_fd_manager().lock();

    // Check if file descriptor is valid
    let fd_entry = match fd_manager.get_fd(dir.fd) {
        Some(entry) => entry,
        None => return code::EBADF.to_errno(),
    };

    // Check if it's a directory
    if fd_entry.flags & O_DIRECTORY == 0 {
        vfslog!("Not a directory fd: {}", dir.fd);
        return code::ENOTDIR.to_errno();
    }

    fd_manager
        .free_fd(dir.fd)
        .map_or_else(|e| e.to_errno(), |_| code::EOK.to_errno())
}
