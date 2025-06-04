//! vfs_api.rs  
//! C API for VFS operations  

use crate::{
    drivers::device::DeviceManager,
    error::{code, Error},
    vfs::{
        vfs_devfs, vfs_fd, vfs_log::*, vfs_manager::*, vfs_mnt::*, vfs_mode::*, vfs_node::*,
        vfs_posix, vfs_tmpfs,
    },
};
use alloc::{slice, string::String, sync::Arc};
use core::ffi::{c_char, c_int, c_ulong, c_void, CStr};
use libc::{EINVAL, S_IFDIR};

/// Initialize the virtual file system  
pub fn vfs_init() -> Result<(), Error> {
    vfslog!("Initializing VFS...");

    // Get VFS manager instance
    let vfs_manager = get_vfs_manager();

    // Register tmpfs filesystem
    let tmpfs = Arc::new(vfs_tmpfs::TmpFileSystem::new());
    vfs_manager.register_fs("tmpfs", tmpfs.clone())?;

    // Mount root filesystem (tmpfs)
    if vfs_posix::mount(None, "/", "tmpfs", 0, None) != 0 {
        vfslog!("Failed to mount root filesystem");
        return Err(code::EAGAIN);
    }

    // Initialize DNode cache
    init_dnode_cache()?;

    // Get root filesystem and DNode cache
    let (fs, _) = match find_filesystem("/") {
        Some(x) => x,
        None => {
            vfslog!("Failed to get root filesystem");
            return Err(code::EAGAIN);
        }
    };

    let dnode_cache = match get_dnode_cache() {
        Some(cache) => cache,
        None => {
            vfslog!("Failed to get DNode cache");
            return Err(code::EAGAIN);
        }
    };

    // create root dir
    let root_inode_no: InodeNo = 1;
    let root_attr = InodeAttr::new(root_inode_no, FileType::Directory, S_IFDIR | 0o755);

    let root_inode = Arc::new(Inode::new(root_attr, fs.clone()));
    let root_dnode = Arc::new(DNode::new(String::from("/"), root_inode, None));
    dnode_cache.insert(root_dnode.clone());
    vfslog!("Created root directory '/'");

    // Verify directory structure
    if dnode_cache.lookup("/").is_none() {
        vfslog!("Failed to verify root directory in cache");
        return Err(code::EAGAIN);
    }

    // Register devfs filesystem
    let devfs = Arc::new(vfs_devfs::DevFileSystem::new(DeviceManager::get()));
    vfs_manager.register_fs("devfs", devfs.clone())?;

    // Mount devfs to /dev
    if vfs_posix::mount(None, "/dev", "devfs", 0, None) != 0 {
        vfslog!("Failed to mount devfs");
        return Err(code::EAGAIN);
    }
    vfslog!("Mounted devfs at '/dev'");

    // Verify mount success
    if let Some(_) = find_filesystem("/dev") {
        vfslog!("devfs mount verified");
    } else {
        vfslog!("Failed to verify devfs mount");
        return Err(code::EAGAIN);
    }

    vfslog!("init stdio");
    let mut fd_manager = vfs_fd::get_fd_manager().lock();
    #[cfg(not(cortex_a))]
    fd_manager.init_stdio()?;

    vfslog!("VFS initialized successfully");
    Ok(())
}

#[no_mangle]
pub extern "C" fn vfs_mount(
    device_name: *const c_char,
    path: *const c_char,
    filesystemtype: *const c_char,
    rwflag: c_ulong,
    data: *const c_void,
) -> c_int {
    if path.is_null() || filesystemtype.is_null() {
        return -EINVAL;
    }

    let path_str = match unsafe { CStr::from_ptr(path).to_str() } {
        Ok(s) => s,
        Err(_) => return -EINVAL,
    };

    let fs_type_str = match unsafe { CStr::from_ptr(filesystemtype).to_str() } {
        Ok(s) => s,
        Err(_) => return -EINVAL,
    };

    let device_str = if device_name.is_null() {
        None
    } else {
        match unsafe { CStr::from_ptr(device_name).to_str() } {
            Ok(s) => Some(s),
            Err(_) => return -EINVAL,
        }
    };

    let data_slice = if data.is_null() {
        None
    } else {
        // Note: Actual usage may require knowing the data length
        // Temporarily passing None here
        None
    };

    vfs_posix::mount(device_str, path_str, fs_type_str, rwflag as u64, data_slice)
}

/// unmount a path
#[no_mangle]
pub extern "C" fn vfs_unmount(path: *const c_char) -> c_int {
    if path.is_null() {
        return -EINVAL;
    }

    let path_str = match unsafe { CStr::from_ptr(path).to_str() } {
        Ok(s) => s,
        Err(_) => return -EINVAL,
    };

    vfs_posix::unmount(path_str)
}

/// Open a file
#[no_mangle]
pub extern "C" fn vfs_open(path: *const c_char, flags: c_int, mode: mode_t) -> c_int {
    if path.is_null() {
        return code::EINVAL.to_errno();
    }

    let file_path = match unsafe { core::ffi::CStr::from_ptr(path).to_str() } {
        Ok(s) => s,
        Err(_) => return code::EINVAL.to_errno(),
    };
    vfs_posix::open(file_path, flags, mode)
}

/// Close a file descriptor
#[no_mangle]
pub extern "C" fn vfs_close(fd: i32) -> i32 {
    vfs_posix::close(fd)
}

/// Read from a file
#[no_mangle]
pub extern "C" fn vfs_read(fd: i32, buf: *mut u8, count: usize) -> isize {
    if buf.is_null() {
        return -EINVAL as isize;
    }

    let slice = unsafe { slice::from_raw_parts_mut(buf, count) };
    vfs_posix::read(fd, slice as *mut _ as *mut c_void, count)
}

/// Write to a file
#[no_mangle]
pub extern "C" fn vfs_write(fd: i32, buf: *const u8, count: usize) -> isize {
    if buf.is_null() {
        return -EINVAL as isize;
    }

    let slice = unsafe { slice::from_raw_parts(buf, count) };
    vfs_posix::write(fd, slice, count)
}

/// Seek in a file
#[no_mangle]
pub extern "C" fn vfs_lseek(fd: i32, offset: i64, whence: i32) -> i64 {
    vfs_posix::lseek(fd, offset, whence)
}

#[no_mangle]
pub extern "C" fn vfs_fcntl(fd: i32, cmd: c_int, args: usize) -> c_int {
    vfs_posix::fcntl(fd, cmd, args)
}

#[no_mangle]
pub extern "C" fn vfs_unlink(path: *const c_char) -> c_int {
    if path.is_null() {
        return code::EINVAL.to_errno();
    }

    let file_path = match unsafe { core::ffi::CStr::from_ptr(path).to_str() } {
        Ok(s) => s,
        Err(_) => return code::EINVAL.to_errno(),
    };
    vfs_posix::unlink(file_path)
}
