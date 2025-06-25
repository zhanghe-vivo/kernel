//! C API for VFS operations  
use crate::{
    error::code,
    vfs::{
        dirent::DirBufferReader,
        fd_manager::get_fd_manager,
        file::{File, FileOps, OpenFlags},
        inode_mode::{InodeFileType, InodeMode},
        mount, path,
        utils::SeekFrom,
    },
};
use alloc::{slice, string::String, sync::Arc};
use core::ffi::{c_char, c_int, c_ulong, c_void, CStr};
use libc;
use log::{debug, warn};

#[no_mangle]
pub extern "C" fn vfs_mount(
    device_name: *const c_char,
    path: *const c_char,
    filesystemtype: *const c_char,
    _rwflag: c_ulong,
    _data: *const c_void,
) -> c_int {
    if path.is_null() || filesystemtype.is_null() {
        return -libc::EINVAL;
    }

    let target = match unsafe { CStr::from_ptr(path).to_str() } {
        Ok(s) => s,
        Err(_) => return -libc::EINVAL,
    };

    let fs_type = match unsafe { CStr::from_ptr(filesystemtype).to_str() } {
        Ok(s) => s,
        Err(_) => return -libc::EINVAL,
    };

    let device = if device_name.is_null() {
        None
    } else {
        match unsafe { CStr::from_ptr(device_name).to_str() } {
            Ok(s) => Some(s),
            Err(_) => return -libc::EINVAL,
        }
    };

    let Some(dir) = path::lookup_path(&target) else {
        warn!("Invalid target path: {}", target);
        return -libc::EINVAL;
    };

    if dir.inode().type_() != InodeFileType::Directory {
        warn!("Target path is not a directory: {}", target);
        return -libc::ENOTDIR;
    }

    if dir.is_mount_point() {
        warn!("[mount] Target path already exists: {}", target);
        return -libc::EEXIST;
    }

    let fs = match mount::get_fs(fs_type, device.unwrap_or("")) {
        Some(fs) => fs,
        None => {
            warn!("Invalid filesystem type: {}", fs_type);
            return -libc::EINVAL;
        }
    };

    match dir.mount(fs.clone()) {
        Ok(_) => {
            debug!("[mount] Successfully mounted {} at {}", fs_type, target);

            let mount_manager = mount::get_mount_manager();
            match mount_manager.add_mount(&dir.get_full_path(), dir.clone(), fs.clone()) {
                Ok(_) => code::EOK.to_errno(),
                Err(e) => e.to_errno(),
            }
        }
        Err(e) => e.to_errno(),
    }
}

/// unmount a path
#[no_mangle]
pub extern "C" fn vfs_unmount(path: *const c_char) -> c_int {
    if path.is_null() {
        return -libc::EINVAL;
    }

    let target = match unsafe { CStr::from_ptr(path).to_str() } {
        Ok(s) => s,
        Err(_) => return -libc::EINVAL,
    };

    let Some(dir) = path::lookup_path(&target) else {
        warn!("Invalid target path: {}", target);
        return -libc::EINVAL;
    };

    match dir.unmount() {
        Ok(_) => {
            debug!("[unmount] Successfully unmounted {}", target);

            // find mount point
            let mount_manager = mount::get_mount_manager();
            match mount_manager.remove_mount(&dir.get_full_path()) {
                Ok(_) => 0,
                Err(e) => e.to_errno(),
            }
        }
        Err(e) => e.to_errno(),
    }
}

/// Open a file
#[no_mangle]
pub extern "C" fn vfs_open(path: *const c_char, flags: c_int, mode: libc::mode_t) -> c_int {
    if path.is_null() {
        return -libc::EINVAL;
    }

    let file_path = match unsafe { CStr::from_ptr(path).to_str() } {
        Ok(s) => s,
        Err(_) => return -libc::EINVAL,
    };
    debug!(
        "vfs_open: path = {}, flags = {}, mode = {}",
        file_path,
        flags_to_string(flags),
        mode
    );

    let file = {
        match path::open_path(file_path, flags, mode) {
            Ok(file) => Arc::new(file),
            Err(e) => return e.to_errno(),
        }
    };

    let mut fd_manager = get_fd_manager().lock();
    let fd = fd_manager.alloc_fd(file);
    fd as i32
}

#[no_mangle]
pub extern "C" fn vfs_creat(path: *const c_char, mode: libc::mode_t) -> c_int {
    let flags = libc::O_CREAT | libc::O_WRONLY | libc::O_TRUNC;
    vfs_open(path, flags, mode)
}

/// Close a file descriptor
#[no_mangle]
pub extern "C" fn vfs_close(fd: i32) -> i32 {
    let file_ops = {
        let mut fd_manager = get_fd_manager().lock();
        let entry = match fd_manager.get_file_ops(fd) {
            Some(entry) => entry,
            None => return -libc::EBADF as i32,
        };
        let _ = fd_manager.free_fd(fd);
        entry
    };

    match file_ops.close() {
        Ok(_) => 0,
        Err(e) => e.to_errno(),
    }
}

/// Read from a file
#[no_mangle]
pub extern "C" fn vfs_read(fd: i32, buf: *mut u8, count: usize) -> isize {
    if buf.is_null() {
        return -libc::EINVAL as isize;
    }

    if count == 0 {
        return 0;
    }

    let file_ops = {
        let fd_manager = get_fd_manager().lock();
        match fd_manager.get_file_ops(fd) {
            Some(ops) => ops,
            None => return -libc::EBADF as isize,
        }
    };

    let slice = unsafe { slice::from_raw_parts_mut(buf, count) };
    match file_ops.read(slice) {
        Ok(n) => n as isize,
        Err(e) => e.to_errno() as isize,
    }
}

/// Write to a file
#[no_mangle]
pub extern "C" fn vfs_write(fd: i32, buf: *const u8, count: usize) -> isize {
    if buf.is_null() {
        return -libc::EINVAL as isize;
    }

    if count == 0 {
        return 0;
    }

    let file_ops = {
        let fd_manager = get_fd_manager().lock();
        match fd_manager.get_file_ops(fd) {
            Some(ops) => ops,
            None => return -libc::EBADF as isize,
        }
    };

    let slice = unsafe { slice::from_raw_parts(buf, count) };
    match file_ops.write(slice) {
        Ok(n) => n as isize,
        Err(e) => e.to_errno() as isize,
    }
}

/// Seek in a file
#[no_mangle]
pub extern "C" fn vfs_lseek(fd: i32, offset: i64, whence: i32) -> i64 {
    debug!(
        "lseek: fd = {}, offset = {}, whence = {}",
        fd, offset, whence
    );
    let seek_from = match whence {
        0 => {
            if offset < 0 {
                return -libc::EINVAL as i64;
            }
            SeekFrom::Start(offset as u64)
        }
        1 => SeekFrom::Current(offset),
        2 => SeekFrom::End(offset),
        _ => return -libc::EINVAL as i64,
    };

    let file_ops = {
        let fd_manager = get_fd_manager().lock();
        match fd_manager.get_file_ops(fd) {
            Some(ops) => ops,
            None => return -libc::EBADF as i64,
        }
    };

    match file_ops.seek(seek_from) {
        Ok(n) => n as i64,
        Err(e) => e.to_errno() as i64,
    }
}

#[no_mangle]
pub extern "C" fn vfs_truncate(path: *const c_char, length: libc::off_t) -> c_int {
    if path.is_null() {
        return -libc::EINVAL;
    }

    let file_path = match unsafe { CStr::from_ptr(path).to_str() } {
        Ok(s) => s,
        Err(_) => return -libc::EINVAL,
    };
    debug!("vfs_truncate: path = {}, length = {}", file_path, length);

    let file = match path::lookup_path(file_path) {
        Some(entry) => entry,
        None => return -libc::EINVAL,
    };
    match file.resize(length as usize) {
        Ok(_) => 0,
        Err(e) => e.to_errno(),
    }
}

#[no_mangle]
pub extern "C" fn vfs_ftruncate(fd: i32, length: libc::off_t) -> c_int {
    debug!("vfs_ftruncate: fd = {}, length = {}", fd, length);

    let file_ops = {
        let fd_manager = get_fd_manager().lock();
        match fd_manager.get_file_ops(fd) {
            Some(ops) => ops,
            None => return -libc::EBADF,
        }
    };

    match file_ops.resize(length as usize) {
        Ok(_) => 0,
        Err(e) => e.to_errno(),
    }
}

#[no_mangle]
pub extern "C" fn vfs_fcntl(fd: i32, cmd: c_int, args: usize) -> c_int {
    debug!("fcntl: fd = {}, cmd = {}, args = {}", fd, cmd, args);
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
            let fd_entry = match fd_manager.get_file_ops(fd) {
                Some(entry) => entry,
                None => return -libc::EBADF,
            };
            if fd_entry.flags().contains(OpenFlags::O_CLOEXEC) {
                FD_CLOEXEC
            } else {
                0
            }
        }
        libc::F_SETFD => {
            let flags = args as c_int;
            if flags & !FD_CLOEXEC != 0 {
                return -libc::ENOSYS;
            }

            let is_cloexec = (args as c_int) & FD_CLOEXEC != 0;

            let fd_manager = get_fd_manager().lock();
            let fd_entry = match fd_manager.get_file_ops(fd) {
                Some(entry) => entry,
                None => return -libc::EBADF,
            };
            if is_cloexec {
                fd_entry.set_flags(fd_entry.flags() | OpenFlags::O_CLOEXEC);
            } else {
                fd_entry.set_flags(fd_entry.flags() & !OpenFlags::O_CLOEXEC);
            }
            0
        }
        libc::F_GETFL => {
            let fd_manager = get_fd_manager().lock();
            let fd_entry = match fd_manager.get_file_ops(fd) {
                Some(entry) => entry,
                None => return -libc::EBADF,
            };
            fd_entry.flags().bits() as c_int
        }
        libc::F_SETFL => {
            // this operation can change only O_NONBLOCK for now
            let fd_manager = get_fd_manager().lock();
            let fd_entry = match fd_manager.get_file_ops(fd) {
                Some(entry) => entry,
                None => return -libc::EBADF,
            };

            let oflags = args as c_int;
            if oflags & libc::O_NONBLOCK == 0 {
                fd_entry.set_flags(fd_entry.flags() & !OpenFlags::O_NONBLOCK);
            } else {
                fd_entry.set_flags(fd_entry.flags() | OpenFlags::O_NONBLOCK);
            }
            0
        }

        _ => return -libc::ENOSYS,
    }
}

#[no_mangle]
pub extern "C" fn vfs_link(
    old_path: *const c_char,
    new_path: *const c_char,
    _flags: c_int,
) -> c_int {
    if old_path.is_null() || new_path.is_null() {
        return -libc::EINVAL;
    }

    let old_path = match unsafe { CStr::from_ptr(old_path).to_str() } {
        Ok(s) => s,
        Err(_) => return -libc::ENOENT,
    };

    let new_path = match unsafe { CStr::from_ptr(new_path).to_str() } {
        Ok(s) => s,
        Err(_) => return -libc::EINVAL,
    };

    if old_path.ends_with('/') {
        warn!("Cannot link to a directory: {}", old_path);
        return -libc::EPERM;
    }

    if new_path.ends_with('/') {
        warn!("new path is a directory: {}", new_path);
        return -libc::ENOENT;
    }

    let old_dentry = match path::lookup_path(old_path) {
        Some(dentry) => dentry,
        None => return -libc::ENOENT,
    };
    let (new_dir, new_name) = match path::find_parent_and_name(new_path) {
        Some(result) => result,
        None => return -libc::ENOENT,
    };

    match new_dir.link(&old_dentry, &new_name) {
        Ok(_) => 0,
        Err(e) => e.to_errno(),
    }
}

#[no_mangle]
pub extern "C" fn vfs_unlink(path: *const c_char) -> c_int {
    if path.is_null() {
        return -libc::EINVAL;
    }

    let file_path = match unsafe { CStr::from_ptr(path).to_str() } {
        Ok(s) => s,
        Err(_) => return -libc::EINVAL,
    };

    if file_path.ends_with('/') {
        warn!("Cannot unlink a directory: {}", file_path);
        return -libc::EISDIR;
    }

    let Some((dir, name)) = path::find_parent_and_name(file_path) else {
        warn!("Invalid path: {}", file_path);
        return -libc::EINVAL;
    };

    match dir.unlink(name) {
        Ok(_) => 0,
        Err(e) => e.to_errno(),
    }
}

#[no_mangle]
pub extern "C" fn vfs_mkdir(path: *const c_char, mode: libc::mode_t) -> i32 {
    if path.is_null() {
        return -libc::EINVAL;
    }

    let file_path = match unsafe { CStr::from_ptr(path).to_str() } {
        Ok(s) => s,
        Err(_) => return -libc::EINVAL,
    };

    let (dir, name) = match path::find_parent_and_name(file_path) {
        Some((dir, name)) => (dir, name),
        None => return -libc::EINVAL,
    };

    match dir.new_child(name, InodeFileType::Directory, InodeMode::from(mode)) {
        Ok(_) => 0,
        Err(e) => e.to_errno(),
    }
}

#[no_mangle]
pub extern "C" fn vfs_rmdir(path: *const c_char) -> c_int {
    if path.is_null() {
        return -libc::EINVAL;
    }

    let file_path = match unsafe { CStr::from_ptr(path).to_str() } {
        Ok(s) => s,
        Err(_) => return -libc::EINVAL,
    };

    if file_path == "/" {
        warn!("Cannot remove root directory");
        return -libc::EBUSY;
    }

    let Some((dir, name)) = path::find_parent_and_name(file_path) else {
        warn!("Invalid path: {}", file_path);
        return -libc::EINVAL;
    };

    match dir.rmdir(name.trim_end_matches('/')) {
        Ok(_) => 0,
        Err(e) => e.to_errno(),
    }
}

#[no_mangle]
pub extern "C" fn vfs_getdents(fd: i32, buf: *mut u8, buf_len: usize) -> c_int {
    let file_ops = {
        let fd_manager = get_fd_manager().lock();
        match fd_manager.get_file_ops(fd) {
            Some(ops) => ops,
            None => return -libc::EBADF,
        }
    };

    let file = match file_ops.downcast_ref::<File>() {
        Some(file) => file,
        None => return -libc::EBADF,
    };
    if file.type_() != InodeFileType::Directory {
        return -libc::ENOTDIR;
    }

    let buf = unsafe { slice::from_raw_parts_mut(buf, buf_len) };
    let mut reader = DirBufferReader::new(buf);

    match file.getdents(&mut reader) {
        Ok(_) => reader.recv_len() as c_int,
        Err(e) => e.to_errno(),
    }
}

#[no_mangle]
pub extern "C" fn vfs_stat(path: *const c_char, buf: *mut libc::statvfs) -> c_int {
    if path.is_null() || buf.is_null() {
        return -libc::EINVAL;
    }

    let path_str = match unsafe { CStr::from_ptr(path).to_str() } {
        Ok(s) => s,
        Err(_) => return -libc::EINVAL,
    };

    let dir_entry = match path::lookup_path(path_str) {
        Some(entry) => entry,
        None => return -libc::EINVAL,
    };
    let fs_info = dir_entry.fs().fs_info();

    unsafe {
        (*buf).f_bsize = fs_info.bsize as u32;
        (*buf).f_frsize = fs_info.frsize as u32;
        (*buf).f_blocks = fs_info.blocks as u64;
        (*buf).f_bfree = fs_info.bfree as u64;
        (*buf).f_bavail = fs_info.bavail as u64;
        (*buf).f_files = fs_info.files as u32;
        (*buf).f_ffree = fs_info.ffree as u32;
        (*buf).f_favail = fs_info.bavail as u32;
        (*buf).f_fsid = fs_info.fsid as u32;
        (*buf).f_flag = fs_info.flags as u32;
        (*buf).f_namemax = fs_info.namelen as u32;
    }

    return 0;
}

#[no_mangle]
pub extern "C" fn vfs_chdir(path: *const c_char) -> c_int {
    if path.is_null() {
        return -libc::EINVAL;
    }

    let path_str = match unsafe { CStr::from_ptr(path).to_str() } {
        Ok(s) => s,
        Err(_) => return -libc::EINVAL,
    };

    let dir_entry = match path::lookup_path(path_str) {
        Some(entry) => entry,
        None => return -libc::EINVAL,
    };

    match path::set_working_dir(dir_entry.clone()) {
        Ok(_) => 0,
        Err(e) => e.to_errno(),
    }
}

#[no_mangle]
pub extern "C" fn vfs_getcwd(buf: *mut c_char, len: usize) -> c_int {
    if buf.is_null() || len == 0 {
        return -libc::EINVAL;
    }

    let cwd = path::get_working_dir();
    let cwd_str = cwd.get_full_path();
    let cwd_str_len = cwd_str.len();
    if cwd_str_len > len - 1 {
        return -libc::ERANGE;
    }
    unsafe {
        core::ptr::copy_nonoverlapping(cwd_str.as_ptr(), buf as *mut u8, cwd_str_len);
        *(buf as *mut u8).add(cwd_str_len) = 0;
    }
    return cwd_str_len as c_int;
}

/// Convert open flags to readable string for debugging
fn flags_to_string(flags: c_int) -> String {
    let mut result = String::new();

    // Check access mode
    match flags & libc::O_ACCMODE {
        x if x == libc::O_RDONLY => result.push_str("O_RDONLY"),
        x if x == libc::O_WRONLY => result.push_str("O_WRONLY"),
        x if x == libc::O_RDWR => result.push_str("O_RDWR"),
        _ => result.push_str("O_UNKNOWN"),
    }

    // Check creation flags
    if flags & libc::O_CREAT != 0 {
        result.push_str("| O_CREAT");
    }
    if flags & libc::O_EXCL != 0 {
        result.push_str("| O_EXCL");
    }
    if flags & libc::O_TRUNC != 0 {
        result.push_str("| O_TRUNC");
    }
    if flags & libc::O_APPEND != 0 {
        result.push_str("| O_APPEND");
    }
    if flags & libc::O_NONBLOCK != 0 {
        result.push_str("| O_NONBLOCK");
    }
    if flags & libc::O_SYNC != 0 {
        result.push_str("| O_SYNC");
    }
    // Add directory-related flags
    if flags & libc::O_DIRECTORY != 0 {
        result.push_str("| O_DIRECTORY");
    }
    if flags & libc::O_NOFOLLOW != 0 {
        result.push_str("| O_NOFOLLOW");
    }
    if flags & libc::O_CLOEXEC != 0 {
        result.push_str("| O_CLOEXEC");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vfs::dirent::{Dirent, DirentType};
    use bluekernel_test_macro::test;
    use libc;

    // Mock data for testing
    const TEST_PATH: *const c_char = b"/test/file.txt\0".as_ptr() as *const c_char;
    const TEST_DIR: *const c_char = b"/test\0".as_ptr() as *const c_char;
    const TEST_SUB_DIR: *const c_char = b"/test/subdir\0".as_ptr() as *const c_char;
    const ROOT_DIR: *const c_char = b"/\0".as_ptr() as *const c_char;

    #[test]
    fn test_open_invalid_path() {
        // Test with null pointer
        let result = vfs_open(core::ptr::null(), libc::O_RDONLY, 0o644);
        assert_eq!(result, code::EINVAL.to_errno());
    }

    #[test]
    fn test_open_create_file() {
        let result = vfs_mkdir(TEST_DIR, 0o755);
        assert_eq!(result, code::EOK.to_errno());

        let fd = vfs_open(TEST_PATH, libc::O_CREAT | libc::O_WRONLY, 0o644);
        assert!(fd > 0);

        let result = vfs_close(fd);
        assert_eq!(result, code::EOK.to_errno());

        let fd = vfs_open(TEST_PATH, libc::O_WRONLY, 0o644);
        assert!(fd > 0);

        let result = vfs_close(fd);
        assert_eq!(result, code::EOK.to_errno());

        let result = vfs_unlink(TEST_PATH);
        assert_eq!(result, code::EOK.to_errno());

        let result = vfs_rmdir(TEST_DIR);
        assert_eq!(result, code::EOK.to_errno());
    }

    #[test]
    fn test_close_invalid_fd() {
        // Test closing invalid file descriptor
        let result = vfs_close(-1);
        assert_eq!(result, code::EBADF.to_errno());

        let result = vfs_close(1000);
        assert_eq!(result, code::EBADF.to_errno());
    }

    #[test]
    fn test_read_invalid_params() {
        // Test with null buffer
        let result = vfs_read(0, core::ptr::null_mut(), 100);
        assert_eq!(result, code::EINVAL.to_errno() as isize);

        // Test with invalid fd
        let mut buffer = [0u8; 100];
        let result = vfs_read(-1, buffer.as_mut_ptr() as *mut u8, 100);
        assert_eq!(result, code::EBADF.to_errno() as isize);
    }

    #[test]
    fn test_write_invalid_fd() {
        let result = vfs_write(-1, b"test".as_ptr() as *const u8, 4);
        assert_eq!(result, code::EBADF.to_errno() as isize);
    }

    #[test]
    fn test_lseek_invalid_params() {
        // Test with invalid file descriptor
        let result = vfs_lseek(-1, 0, libc::SEEK_SET);
        assert_eq!(result, code::EBADF.to_errno() as i64);

        // Test with invalid whence
        let result = vfs_lseek(0, 0, 999);
        assert_eq!(result, code::EINVAL.to_errno() as i64);

        // Test with negative offset for SEEK_SET
        let result = vfs_lseek(0, -1, libc::SEEK_SET);
        assert_eq!(result, code::EINVAL.to_errno() as i64);
    }

    #[test]
    fn test_mkdir_invalid_path() {
        // Test with empty path
        let result = vfs_mkdir(core::ptr::null(), 0o755);
        assert_eq!(result, code::EINVAL.to_errno());

        // Test with root path
        let result = vfs_mkdir(ROOT_DIR, 0o755);
        assert_eq!(result, code::EEXIST.to_errno());
    }

    #[test]
    fn test_rmdir_invalid_path() {
        // Test with empty path
        let result = vfs_rmdir(core::ptr::null());
        assert_eq!(result, code::EINVAL.to_errno());

        // Test with non-existent path
        let result = vfs_rmdir(TEST_DIR);
        assert_eq!(result, code::ENOENT.to_errno());
    }

    #[test]
    fn test_dir() {
        let result = vfs_open(TEST_DIR, libc::O_RDONLY, 0o755);
        assert_eq!(result, code::ENOENT.to_errno());

        let result = vfs_mkdir(TEST_SUB_DIR, 0o755);
        assert_eq!(result, code::EINVAL.to_errno());

        let result = vfs_mkdir(TEST_DIR, 0o755);
        assert_eq!(result, code::EOK.to_errno());

        let result = vfs_mkdir(TEST_SUB_DIR, 0o755);
        assert_eq!(result, code::EOK.to_errno());

        let dir = vfs_open(TEST_SUB_DIR, libc::O_RDONLY, 0o755);
        assert!(dir > 0);

        let mut buf = [0u8; 256];
        let len = vfs_getdents(dir, buf.as_mut_ptr() as *mut u8, buf.len());
        assert!(len > 0);

        let result = vfs_close(dir);
        assert_eq!(result, code::EOK.to_errno());

        let result = vfs_rmdir(TEST_DIR);
        assert_eq!(result, code::ENOTEMPTY.to_errno());

        let result = vfs_rmdir(TEST_SUB_DIR);
        assert_eq!(result, code::EOK.to_errno());

        let result = vfs_rmdir(TEST_DIR);
        assert_eq!(result, code::EOK.to_errno());
    }

    #[test]
    fn test_fcntl_invalid_params() {
        // Test F_GETFD with invalid fd
        let result = vfs_fcntl(-1, libc::F_GETFD, 0);
        assert_eq!(result, code::EBADF.to_errno());

        // Test F_SETFD with invalid fd
        let result = vfs_fcntl(-1, libc::F_SETFD, 0);
        assert_eq!(result, code::EBADF.to_errno());

        // Test F_SETFD with invalid flags
        let result = vfs_fcntl(0, libc::F_SETFD, 256); // flags > u8::MAX
        assert_eq!(result, code::ENOSYS.to_errno());

        // Test unsupported command
        let result = vfs_fcntl(0, 999, 0);
        assert_eq!(result, code::ENOSYS.to_errno());
    }

    #[test]
    fn test_fcntl_dupfd() {
        // Test F_DUPFD with invalid source fd
        let result = vfs_fcntl(-1, libc::F_DUPFD, 0);
        assert_eq!(result, code::EBADF.to_errno());

        // Test F_DUPFD_CLOEXEC with invalid source fd
        let result = vfs_fcntl(-1, libc::F_DUPFD_CLOEXEC, 0);
        assert_eq!(result, code::EBADF.to_errno());
    }

    #[test]
    fn test_mount_invalid_params() {
        // Test with invalid target path
        let result = vfs_mount(
            core::ptr::null(),
            core::ptr::null(),
            core::ptr::null(),
            0,
            core::ptr::null(),
        );
        assert_eq!(result, code::EINVAL.to_errno());
    }

    #[test]
    fn test_getdents_current_dir() {
        let dir = vfs_open(b".\0".as_ptr() as *const c_char, libc::O_RDONLY, 0o755);
        assert!(dir > 0);

        let mut buf = [0u8; 512];
        // Print return value of each readdir call
        let len = vfs_getdents(dir, buf.as_mut_ptr() as *mut u8, buf.len());
        assert!(len > 0);
        let mut next_entry = 0;
        while next_entry < len as usize {
            let entry = unsafe { Dirent::from_buf_ref(&buf[next_entry..]) };
            if entry.type_() == DirentType::Dir {
                println!(
                    "[VFS Test DirctoryTree]: Found directory: {} {} {}",
                    entry.ino(),
                    entry.off(),
                    entry.name().unwrap().to_string_lossy()
                );
            } else {
                println!(
                    "[VFS Test DirctoryTree]: Found file: {} {} {}",
                    entry.ino(),
                    entry.off(),
                    entry.name().unwrap().to_string_lossy()
                );
            }
            next_entry += entry.reclen() as usize;
        }

        // Close directory
        vfs_close(dir);
    }

    #[test]
    fn test_getdents_parent_dir() {
        let dir = vfs_open(b"..\0".as_ptr() as *const c_char, libc::O_RDONLY, 0o755);
        assert!(dir > 0);

        let mut buf = [0u8; 512];
        // Print return value of each readdir call
        let len = vfs_getdents(dir, buf.as_mut_ptr() as *mut u8, buf.len());
        assert!(len > 0);
        let mut next_entry = 0;
        while next_entry < len as usize {
            let entry = unsafe { Dirent::from_buf_ref(&buf[next_entry..]) };
            if entry.type_() == DirentType::Dir {
                println!(
                    "[VFS Test DirctoryTree]: Found directory: {} {} {}",
                    entry.ino(),
                    entry.off(),
                    entry.name().unwrap().to_string_lossy()
                );
            } else {
                println!(
                    "[VFS Test DirctoryTree]: Found file: {} {} {}",
                    entry.ino(),
                    entry.off(),
                    entry.name().unwrap().to_string_lossy()
                );
            }
            next_entry += entry.reclen() as usize;
        }

        // Close directory
        vfs_close(dir);
    }

    #[test]
    fn test_chdir() {
        let result = vfs_mkdir(TEST_DIR, 0o755);
        assert_eq!(result, code::EOK.to_errno());

        let result = vfs_chdir(TEST_DIR);
        assert_eq!(result, code::EOK.to_errno());
        let mut buf = [0u8; 256];
        let result = vfs_getcwd(buf.as_mut_ptr() as *mut c_char, buf.len());
        assert!(result > 0);
        unsafe {
            let path = CStr::from_ptr(buf.as_ptr() as *const c_char);
            assert_eq!(path, CStr::from_ptr(TEST_DIR));
        }

        let result = vfs_chdir(TEST_SUB_DIR);
        assert_eq!(result, code::EINVAL.to_errno());

        let result = vfs_rmdir(TEST_DIR);
        assert_eq!(result, code::EOK.to_errno());
    }

    #[test]
    fn test_truncate_invalid_params() {
        // Test with null path
        let result = vfs_truncate(core::ptr::null(), 100);
        assert_eq!(result, code::EINVAL.to_errno());

        // Test with non-existent path
        let result = vfs_truncate(TEST_PATH, 100);
        assert_eq!(result, code::EINVAL.to_errno());
    }

    #[test]
    fn test_truncate_file() {
        // Create directory and file
        let result = vfs_mkdir(TEST_DIR, 0o755);
        assert_eq!(result, code::EOK.to_errno());

        let fd = vfs_open(TEST_PATH, libc::O_CREAT | libc::O_WRONLY, 0o644);
        assert!(fd > 0);

        // Write some data to file
        let test_data = b"Hello, World!";
        let write_result = vfs_write(fd, test_data.as_ptr() as *const u8, test_data.len());
        assert_eq!(write_result, test_data.len() as isize);

        // Close file
        let result = vfs_close(fd);
        assert_eq!(result, code::EOK.to_errno());

        // Test truncate to smaller size
        let result = vfs_truncate(TEST_PATH, 5);
        assert_eq!(result, code::EOK.to_errno());

        // Open file and read to verify truncation
        let fd = vfs_open(TEST_PATH, libc::O_RDONLY, 0o644);
        assert!(fd > 0);

        let mut buffer = [0u8; 20];
        let read_result = vfs_read(fd, buffer.as_mut_ptr() as *mut u8, buffer.len());
        assert_eq!(read_result, 5);

        // Verify content is truncated
        assert_eq!(&buffer[0..5], b"Hello");

        // Close file
        let result = vfs_close(fd);
        assert_eq!(result, code::EOK.to_errno());

        // Test truncate to larger size
        let result = vfs_truncate(TEST_PATH, 20);
        assert_eq!(result, code::EOK.to_errno());

        // Open file and read to verify expansion
        let fd = vfs_open(TEST_PATH, libc::O_RDONLY, 0o644);
        assert!(fd > 0);

        let mut buffer = [0u8; 25];
        let read_result = vfs_read(fd, buffer.as_mut_ptr() as *mut u8, buffer.len());
        assert_eq!(read_result, 20);

        // Verify original content is preserved, rest is zero-filled
        assert_eq!(&buffer[0..5], b"Hello");
        assert_eq!(&buffer[5..20], &[0u8; 15]);

        // Close file
        let result = vfs_close(fd);
        assert_eq!(result, code::EOK.to_errno());

        // Cleanup
        let result = vfs_unlink(TEST_PATH);
        assert_eq!(result, code::EOK.to_errno());

        let result = vfs_rmdir(TEST_DIR);
        assert_eq!(result, code::EOK.to_errno());
    }

    #[test]
    fn test_ftruncate_invalid_params() {
        // Test with invalid file descriptor
        let result = vfs_ftruncate(-1, 100);
        assert_eq!(result, code::EBADF.to_errno());

        let result = vfs_ftruncate(1000, 100);
        assert_eq!(result, code::EBADF.to_errno());
    }

    #[test]
    fn test_ftruncate_file() {
        // Create directory and file
        let result = vfs_mkdir(TEST_DIR, 0o755);
        assert_eq!(result, code::EOK.to_errno());

        let fd = vfs_open(TEST_PATH, libc::O_CREAT | libc::O_RDWR, 0o644);
        assert!(fd > 0);

        // Write some data to file
        let test_data = b"Hello, World!";
        let write_result = vfs_write(fd, test_data.as_ptr() as *const u8, test_data.len());
        assert_eq!(write_result, test_data.len() as isize);

        // Test ftruncate to smaller size
        let result = vfs_ftruncate(fd, 5);
        assert_eq!(result, code::EOK.to_errno());

        // Seek to beginning and read to verify truncation
        let seek_result = vfs_lseek(fd, 0, libc::SEEK_SET);
        assert_eq!(seek_result, 0);

        let mut buffer = [0u8; 20];
        let read_result = vfs_read(fd, buffer.as_mut_ptr() as *mut u8, buffer.len());
        assert_eq!(read_result, 5);

        // Verify content is truncated
        assert_eq!(&buffer[0..5], b"Hello");

        // Test ftruncate to larger size
        let result = vfs_ftruncate(fd, 20);
        assert_eq!(result, code::EOK.to_errno());

        // Seek to beginning and read to verify expansion
        let seek_result = vfs_lseek(fd, 0, libc::SEEK_SET);
        assert_eq!(seek_result, 0);

        let mut buffer = [0u8; 25];
        let read_result = vfs_read(fd, buffer.as_mut_ptr() as *mut u8, buffer.len());
        assert_eq!(read_result, 20);

        // Verify original content is preserved, rest is zero-filled
        assert_eq!(&buffer[0..5], b"Hello");
        assert_eq!(&buffer[5..20], &[0u8; 15]);

        // Close file
        let result = vfs_close(fd);
        assert_eq!(result, code::EOK.to_errno());

        // Cleanup
        let result = vfs_unlink(TEST_PATH);
        assert_eq!(result, code::EOK.to_errno());

        let result = vfs_rmdir(TEST_DIR);
        assert_eq!(result, code::EOK.to_errno());
    }

    #[test]
    fn test_truncate_directory() {
        // Create directory
        let result = vfs_mkdir(TEST_DIR, 0o755);
        assert_eq!(result, code::EOK.to_errno());

        // Try to truncate directory (should fail)
        let result = vfs_truncate(TEST_DIR, 100);
        assert_eq!(result, code::EISDIR.to_errno());

        // Cleanup
        let result = vfs_rmdir(TEST_DIR);
        assert_eq!(result, code::EOK.to_errno());
    }

    #[test]
    fn test_ftruncate_readonly_file() {
        // Create directory and file
        let result = vfs_mkdir(TEST_DIR, 0o755);
        assert_eq!(result, code::EOK.to_errno());

        let fd = vfs_open(TEST_PATH, libc::O_CREAT | libc::O_WRONLY, 0o644);
        assert!(fd > 0);

        // Write some data
        let test_data = b"Hello, World!";
        let write_result = vfs_write(fd, test_data.as_ptr() as *const u8, test_data.len());
        assert_eq!(write_result, test_data.len() as isize);

        // Close file
        let result = vfs_close(fd);
        assert_eq!(result, code::EOK.to_errno());

        // Open file as read-only
        let fd = vfs_open(TEST_PATH, libc::O_RDONLY, 0o644);
        assert!(fd > 0);

        // Try to truncate read-only file (should fail)
        let result = vfs_ftruncate(fd, 5);
        assert_eq!(result, code::EACCES.to_errno());

        // Close file
        let result = vfs_close(fd);
        assert_eq!(result, code::EOK.to_errno());

        // Cleanup
        let result = vfs_unlink(TEST_PATH);
        assert_eq!(result, code::EOK.to_errno());

        let result = vfs_rmdir(TEST_DIR);
        assert_eq!(result, code::EOK.to_errno());
    }
}
