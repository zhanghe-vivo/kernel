//! vfs_tmpfs.rs  
#![allow(dead_code)]

use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    vec::Vec,
};
use core::{
    cmp::min,
    sync::atomic::{AtomicBool, Ordering},
};
use spin::RwLock as SpinRwLock;

use crate::{
    error::{code, Error},
    vfs::{
        vfs_dirent::*,
        vfs_log::*,
        vfs_mode::*,
        vfs_node::{FileType, InodeAttr, InodeNo},
        vfs_path::*,
        vfs_traits::{FileOperationTrait, FileSystemTrait},
    },
};
use libc::{O_APPEND, O_DIRECTORY, O_RDWR, O_TRUNC, O_WRONLY, S_IFDIR, S_IFLNK, S_IFMT, S_IFREG};

// File access permissions
const R_OK: u32 = 4; // Read permission
const W_OK: u32 = 2; // Write permission
const X_OK: u32 = 1; // Execute permission

/// Inode in temporary filesystem
struct TmpInode {
    attr: InodeAttr,
    data: Vec<u8>,
    offset: usize,
}

/// Temporary filesystem implementation
#[cfg_attr(feature = "cbindgen", no_mangle)]
pub struct TmpFileSystem {
    mounted: AtomicBool,
    mount_point: SpinRwLock<String>,
    // Inode table
    inodes: SpinRwLock<BTreeMap<InodeNo, TmpInode>>,
    // Directory entry table (parent inode number, filename) -> child inode number
    dentries: SpinRwLock<BTreeMap<(InodeNo, String), InodeNo>>,
    // Next available inode number
    next_inode_no: SpinRwLock<InodeNo>,
}

impl TmpFileSystem {
    pub fn new() -> Self {
        vfslog!("[tmpfs] Creating new tmpfs instance");

        TmpFileSystem {
            mounted: AtomicBool::new(false),
            mount_point: SpinRwLock::new(String::new()),
            inodes: SpinRwLock::new(BTreeMap::new()),
            dentries: SpinRwLock::new(BTreeMap::new()),
            next_inode_no: SpinRwLock::new(1), // Start from 1, will be used for root directory
        }
    }

    fn check_mounted(&self) -> Result<(), Error> {
        if !self.mounted.load(Ordering::Relaxed) {
            return Err(code::EAGAIN);
        }
        Ok(())
    }

    /// Allocate new inode number
    fn alloc_inode_no(&self) -> InodeNo {
        let mut next_inode_no = self.next_inode_no.write();
        let inode_no = *next_inode_no;
        *next_inode_no += 1;
        inode_no
    }

    // Convert path to inode number
    fn path_to_inode(&self, path: &str) -> Result<InodeNo, Error> {
        if path.is_empty() {
            return Err(code::EINVAL);
        }

        let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let mut current_inode_no = 1; // Start from root directory

        for component in components {
            let dentries = self.dentries.read();
            current_inode_no = match dentries.get(&(current_inode_no, component.to_string())) {
                Some(&inode_no) => inode_no,
                None => return Err(code::ENOENT),
            };
        }

        Ok(current_inode_no)
    }

    /// Look up inode by path
    fn lookup_path(&self, path: &str) -> Result<InodeNo, Error> {
        if !is_valid_path(path) {
            vfslog!("[tmpfs] lookup_path: Invalid path: {}", path);
            return Err(code::EINVAL);
        }

        // Normalize path
        let path = normalize_path(path).ok_or(code::EINVAL)?;

        // Get mount point
        let mount_point = self.mount_point.read();
        if mount_point.is_empty() {
            vfslog!("[tmpfs] lookup_path: Filesystem not mounted");
            return Err(code::EINVAL);
        }

        // Check if path is under mount point
        if !path.starts_with(&*mount_point) {
            vfslog!(
                "[tmpfs] lookup_path: Path {} not under mount point {}",
                path,
                *mount_point
            );
            return Err(code::EINVAL);
        }

        // If path is the mount point itself
        if path == *mount_point {
            // Find root directory inode
            let inodes = self.inodes.read();
            let root_entry = inodes
                .iter()
                .find(|(_, inode)| inode.attr.file_type == FileType::Directory)
                .ok_or(code::ENOENT)?;
            return Ok(*root_entry.0);
        }

        // Get path relative to mount point
        let rel_path = &path[mount_point.len()..];
        if rel_path.is_empty() || rel_path == "/" {
            // If relative path is empty or only /, return root directory inode
            let inodes = self.inodes.read();
            let root_entry = inodes
                .iter()
                .find(|(_, inode)| inode.attr.file_type == FileType::Directory)
                .ok_or(code::ENOENT)?;
            return Ok(*root_entry.0);
        }

        // Find root directory inode
        let root_inode_no = {
            let inodes = self.inodes.read();
            let root_entry = inodes
                .iter()
                .find(|(_, inode)| inode.attr.file_type == FileType::Directory)
                .ok_or(code::ENOENT)?;
            *root_entry.0
        };

        // Traverse path from root directory
        let mut current_inode_no = root_inode_no;
        for component in rel_path
            .trim_start_matches('/')
            .split('/')
            .filter(|s| !s.is_empty())
        {
            let dentries = self.dentries.read();
            match dentries.get(&(current_inode_no, component.to_string())) {
                Some(&inode_no) => current_inode_no = inode_no,
                None => {
                    vfslog!(
                        "[tmpfs] lookup_path: Component {} not found under inode {}",
                        component,
                        current_inode_no
                    );
                    return Err(code::ENOENT);
                }
            }
        }

        // vfslog!(
        //     "[tmpfs] lookup_path: Found inode {} for path {}",
        //     current_inode_no,
        //     path
        // );
        Ok(current_inode_no)
    }

    /// Look up parent directory inode and filename
    fn lookup_parent(&self, path: &str) -> Result<(InodeNo, String), Error> {
        // Split path
        let (parent_path, name) = split_path(path).ok_or(code::EINVAL)?;

        // Find parent directory inode
        let parent_inode_no = self.lookup_path(parent_path)?;

        Ok((parent_inode_no, name.to_string()))
    }
}

impl FileOperationTrait for TmpFileSystem {
    fn open(&self, path: &str, flags: i32) -> Result<InodeNo, Error> {
        self.check_mounted()?;

        // Parse path to get inode
        let inode_no = self.path_to_inode(path)?;

        let mut inodes = self.inodes.write();
        let inode = match inodes.get_mut(&inode_no) {
            Some(inode) => inode,
            None => return Err(code::ENOENT),
        };

        // Handle O_DIRECTORY flag
        if (flags & O_DIRECTORY) != 0 {
            // If O_DIRECTORY is specified but the path is not a directory, return error
            if inode.attr.file_type != FileType::Directory {
                vfslog!("[tmpfs] open: O_DIRECTORY specified but path is not a directory");
                return Err(code::ENOTDIR);
            }
        } else if inode.attr.file_type == FileType::Directory {
            // If opening a directory but not specifying O_DIRECTORY, also return error
            vfslog!("[tmpfs] open: Opening directory without O_DIRECTORY flag");
            return Err(code::EISDIR);
        }

        // Check file permissions
        let access_mode = if flags & O_RDWR != 0 {
            R_OK | W_OK
        } else if flags & O_WRONLY != 0 {
            W_OK
        } else {
            R_OK
        };

        // Simple permission check
        if inode.attr.mode & access_mode == 0 {
            return Err(code::EACCES);
        }

        // Handle open flags
        if flags & O_TRUNC != 0 {
            if inode.attr.file_type == FileType::Regular {
                // Only regular files can be truncated
                inode.data.clear();
                inode.attr.size = 0;
            }
        }

        // Set initial offset
        if flags & O_APPEND != 0 {
            inode.offset = inode.attr.size;
        } else {
            inode.offset = 0;
        }

        vfslog!(
            "[tmpfs] Successfully opened {} with flags {:x} inode_no {}",
            path,
            flags,
            inode_no
        );
        Ok(inode_no) // Return inode number
    }

    fn close(&self, inode_no: InodeNo) -> Result<(), Error> {
        self.check_mounted()?;
        let mut inodes = self.inodes.write();
        match inodes.get_mut(&inode_no) {
            Some(inode) => inode,
            None => return Err(code::ENOENT),
        };

        vfslog!("[tmpfs] close: inode_no = {}", inode_no);
        Ok(())
    }

    fn read(&self, inode_no: InodeNo, buf: &mut [u8], offset: &mut usize) -> Result<usize, Error> {
        self.check_mounted()?;
        let inodes = self.inodes.read();
        let inode = match inodes.get(&inode_no) {
            Some(inode) => inode,
            None => return Err(code::ENOENT),
        };

        // Use input offset parameter
        let available = (inode.attr.size).saturating_sub(*offset);
        if available == 0 {
            return Ok(0);
        }

        let read_size = min(available, buf.len());
        let start = *offset;
        let end = start + read_size;

        buf[..read_size].copy_from_slice(&inode.data[start..end]);

        // Update input offset
        *offset += read_size;

        Ok(read_size)
    }

    fn write(&self, inode_no: InodeNo, buf: &[u8], offset: &mut usize) -> Result<usize, Error> {
        self.check_mounted()?;
        let mut inodes = self.inodes.write();
        let inode = match inodes.get_mut(&inode_no) {
            Some(inode) => inode,
            None => return Err(code::ENOENT),
        };

        let write_pos = *offset as usize;
        let write_end = write_pos + buf.len();
        // Ensure data buffer is large enough
        if write_end > inode.data.len() {
            inode.data.resize(write_end, 0);
        }
        // Write data
        inode.data[write_pos..write_end].copy_from_slice(buf);
        // Update input offset and file size
        *offset += buf.len();
        inode.attr.size = inode.data.len();

        Ok(buf.len())
    }

    fn get_offset(&self, inode_no: InodeNo) -> Result<usize, Error> {
        self.check_mounted()?;
        let inodes = self.inodes.read();
        match inodes.get(&inode_no) {
            Some(inode) => Ok(inode.offset),
            None => Err(code::ENOENT),
        }
    }

    fn seek(&self, inode_no: InodeNo, offset: usize, whence: i32) -> Result<usize, Error> {
        self.check_mounted()?;
        let mut inodes = self.inodes.write();
        let inode = match inodes.get_mut(&inode_no) {
            Some(inode) => inode,
            None => return Err(code::ENOENT),
        };

        // Calculate new offset
        let new_offset = match whence {
            SEEK_SET => offset,
            SEEK_CUR => {
                // Offset from current position
                let current = inode.offset as i64;
                match current.checked_add(offset as i64) {
                    Some(pos) if pos >= 0 => pos as usize,
                    _ => return Err(code::EINVAL),
                }
            }
            SEEK_END => {
                // Offset from end of file
                let end = inode.attr.size as i64;
                match end.checked_add(offset as i64) {
                    Some(pos) if pos >= 0 => pos as usize,
                    _ => return Err(code::EINVAL),
                }
            }
            _ => return Err(code::EINVAL),
        };

        // Update file offset
        inode.offset = new_offset;
        // Return new offset
        Ok(new_offset)
    }

    fn size(&self, inode_no: InodeNo) -> Result<usize, Error> {
        self.check_mounted()?;
        let inodes = self.inodes.read();
        match inodes.get(&inode_no) {
            Some(inode) => Ok(inode.attr.size),
            None => Err(code::ENOENT),
        }
    }

    fn flush(&self, inode_no: InodeNo) -> Result<(), Error> {
        self.check_mounted()?;
        let inodes = self.inodes.read();
        match inodes.get(&inode_no) {
            Some(_) => Ok(()),
            None => Err(code::ENOENT),
        }
    }

    fn fsync(&self, inode_no: InodeNo) -> Result<(), Error> {
        self.check_mounted()?;
        let inodes = self.inodes.read();
        match inodes.get(&inode_no) {
            Some(_) => Ok(()),
            None => Err(code::ENOENT),
        }
    }

    fn truncate(&self, inode_no: InodeNo, size: usize) -> Result<(), Error> {
        self.check_mounted()?;
        let mut inodes = self.inodes.write();
        let inode = match inodes.get_mut(&inode_no) {
            Some(inode) => inode,
            None => return Err(code::ENOENT),
        };

        inode.data.resize(size, 0);
        inode.attr.size = size;

        Ok(())
    }

    fn getdents(
        &self,
        inode_no: InodeNo,
        offset: usize,
        dirents: &mut Vec<Dirent>,
        count: usize,
    ) -> Result<usize, Error> {
        vfslog!(
            "[tmpfs] getdents: start - inode_no={}, offset={}, count={}",
            inode_no,
            offset,
            count
        );

        // Check if filesystem is mounted
        self.check_mounted()?;

        // Check if directory exists and is directory type
        let inodes = self.inodes.read();
        let dir_inode = match inodes.get(&inode_no) {
            Some(inode) => inode,
            None => return Err(code::ENOENT),
        };

        if dir_inode.attr.file_type != FileType::Directory {
            return Err(code::ENOTDIR);
        }

        // Get directory entries
        let dentries = self.dentries.read();
        // Collect all entries in current directory
        let mut all_entries = Vec::new();
        // Add actual directory entries
        for ((parent, name), &child_inode_no) in dentries.iter() {
            if *parent == inode_no {
                let d_type = if let Some(child_inode_node) = inodes.get(&child_inode_no) {
                    match child_inode_node.attr.file_type {
                        FileType::Regular => DT_REG,
                        FileType::Directory => DT_DIR,
                        FileType::SymLink => DT_LNK,
                        FileType::CharDevice => DT_CHR,
                        FileType::BlockDevice => DT_BLK,
                    }
                } else {
                    DT_UNKNOWN
                };

                all_entries.push(Dirent::new(d_type, name.clone()));
            }
        }

        // Sort by name
        all_entries.sort_by(|a, b| a.name.cmp(&b.name));

        let skip_entries = offset as usize;
        // vfslog!(
        //     "[tmpfs] getdents: total_entries={}, skip_entries={}",
        //     all_entries.len(),
        //     skip_entries
        // );

        // if skip_entries >= all_entries.len() {
        //     vfslog!("[tmpfs] getdents: no more entries (offset too large)");
        //     return Err(code::EFAULT);
        // }

        // Get entries to return
        let entries_to_write = min(count, all_entries.len() - skip_entries);
        if entries_to_write == 0 {
            return Err(code::ENOTDIR);
        }

        // Clear input Vec and extend new entries
        dirents.clear();
        dirents.extend(
            all_entries
                .into_iter()
                .skip(skip_entries)
                .take(entries_to_write),
        );

        // vfslog!(
        //     "[tmpfs] getdents: successfully returned {} entries",
        //     entries_to_write
        // );

        Ok(entries_to_write)
    }
}

impl FileSystemTrait for TmpFileSystem {
    fn mount(
        &self,
        _source: &str,
        target: &str,
        _flags: u64,
        _data: Option<&[u8]>,
    ) -> Result<(), Error> {
        vfslog!("[tmpfs] mount: target = {}", target);

        // Check if already mounted
        if self.mounted.load(Ordering::Relaxed) {
            vfslog!("[tmpfs] Already mounted");
            return Err(code::EEXIST);
        }
        // Check mount point path
        if target.is_empty() {
            vfslog!("[tmpfs] Empty mount point");
            return Err(code::EINVAL);
        }
        // Normalize mount point path
        let normalized_target = match normalize_path(target) {
            Some(path) => path,
            None => {
                vfslog!("[tmpfs] Invalid mount point path");
                return Err(code::EINVAL);
            }
        };
        // Allocate root directory inode number
        let root_inode_no = self.alloc_inode_no();
        vfslog!(
            "[tmpfs] Allocated root directory inode_no = {}",
            root_inode_no
        );
        // Create root directory inode
        let root_attr = InodeAttr {
            inode_no: root_inode_no,
            size: 0,
            blocks: 0,
            atime: 0,
            mtime: 0,
            ctime: 0,
            file_type: FileType::Directory,
            mode: 0o755 | S_IFDIR,
            nlinks: 1,
            uid: 0,
            gid: 0,
        };
        // Initialize filesystem state
        {
            let mut inodes = self.inodes.write();
            let mut dentries = self.dentries.write();
            // Clear existing data
            inodes.clear();
            dentries.clear();
            // Insert root directory inode
            inodes.insert(
                root_inode_no,
                TmpInode {
                    attr: root_attr,
                    data: Vec::new(),
                    offset: 0,
                },
            );
            // Add . and .. entries for root directory
            dentries.insert((root_inode_no, ".".to_string()), root_inode_no);
            dentries.insert((root_inode_no, "..".to_string()), root_inode_no);

            vfslog!("[tmpfs] Created root directory");
        }
        // Set mount point and state
        let mount_path = normalized_target.clone(); // Clone for log output
        *self.mount_point.write() = normalized_target;
        self.mounted.store(true, Ordering::Relaxed);

        vfslog!("[tmpfs] Successfully mounted at {}", mount_path);
        Ok(())
    }

    fn unmount(&self, target: &str) -> Result<(), Error> {
        if !self.mounted.load(Ordering::Relaxed) {
            return Err(code::EAGAIN);
        }

        if target != *self.mount_point.read() {
            return Err(code::EINVAL);
        }

        self.mounted.store(false, Ordering::Relaxed);
        self.mount_point.write().clear();
        self.inodes.write().clear();
        self.dentries.write().clear();
        *self.next_inode_no.write() = 1;

        Ok(())
    }

    fn create_inode(&self, path: &str, mode: u32) -> Result<InodeAttr, Error> {
        vfslog!("[tmpfs] create_inode: path = {}, mode = {:o}", path, mode);

        // Check mount state
        self.check_mounted()?;
        // Check path validity
        if !is_valid_path(path) {
            vfslog!("[tmpfs] create_inode: invalid path");
            return Err(code::EINVAL);
        }
        // Normalize path
        let path = normalize_path(path).ok_or(code::EINVAL)?;
        // vfslog!("[tmpfs] create_inode: normalized path = {}", path);

        // Check if path is under mount point
        let mount_point = self.mount_point.read();
        if !path.starts_with(&*mount_point) {
            return Err(code::EINVAL);
        }
        // Get parent directory path and filename
        let (parent_path, name) = split_path(&path).ok_or(code::EINVAL)?;
        // Find parent directory inode
        let parent_inode_no = self.lookup_path(parent_path)?;
        // Check parent directory
        let inodes = self.inodes.read();
        let parent_inode_node = inodes.get(&parent_inode_no).ok_or(code::ENOENT)?;

        if parent_inode_node.attr.file_type != FileType::Directory {
            return Err(code::ENOTDIR);
        }
        // Check if filename already exists
        if self
            .dentries
            .read()
            .contains_key(&(parent_inode_no, name.to_string()))
        {
            return Err(code::EEXIST);
        }
        drop(inodes);
        // Create new inode
        let inode_no = self.alloc_inode_no();
        vfslog!("[tmpfs] Allocated new inode: {}", inode_no);

        let file_type = if mode & S_IFMT == S_IFDIR {
            FileType::Directory
        } else if mode & S_IFMT == S_IFREG {
            FileType::Regular
        } else if mode & S_IFMT == S_IFLNK {
            FileType::SymLink
        } else {
            FileType::Regular // Default to regular file
        };
        let attr = InodeAttr {
            inode_no,
            size: 0,
            blocks: 0,
            atime: 0,
            mtime: 0,
            ctime: 0,
            file_type,
            mode,
            nlinks: 1,
            uid: 0,
            gid: 0,
        };
        // Add inode and directory entry
        self.inodes.write().insert(
            inode_no,
            TmpInode {
                attr: attr.clone(),
                data: Vec::new(),
                offset: 0,
            },
        );
        self.dentries
            .write()
            .insert((parent_inode_no, name.to_string()), inode_no);
        // If directory, add . and .. entries
        if file_type == FileType::Directory {
            let mut dentries = self.dentries.write();
            dentries.insert((inode_no, ".".to_string()), inode_no);
            dentries.insert((inode_no, "..".to_string()), parent_inode_no);
        }

        Ok(attr)
    }

    fn remove_inode(&self, path: &str) -> Result<(), Error> {
        self.check_mounted()?;

        if !is_valid_path(path) {
            return Err(code::EINVAL);
        }
        // Normalize path
        let path = normalize_path(path).ok_or(code::EINVAL)?;
        // Check if path is under mount point
        let mount_point = self.mount_point.read();
        if !path.starts_with(&*mount_point) {
            return Err(code::EINVAL);
        }
        // Get parent directory path and filename
        let (parent_path, name) = split_path(&path).ok_or(code::EINVAL)?;
        // Find parent directory and target file inode
        let parent_inode_no = self.lookup_path(parent_path)?;
        let target_inode_no = self
            .dentries
            .read()
            .get(&(parent_inode_no, name.to_string()))
            .copied()
            .ok_or(code::ENOENT)?;
        // Check target type
        let is_directory = {
            let inodes = self.inodes.read();
            let target = inodes.get(&target_inode_no).ok_or(code::ENOENT)?;
            target.attr.file_type == FileType::Directory
        };
        // If directory, check if empty
        if is_directory {
            let dentries = self.dentries.read();
            let has_children = dentries
                .iter()
                .any(|((p, n), _)| *p == target_inode_no && n != "." && n != "..");
            if has_children {
                return Err(code::ENOTEMPTY);
            }
        }
        // Remove directory entry
        self.dentries
            .write()
            .remove(&(parent_inode_no, name.to_string()));
        // If directory, remove . and .. entries
        if is_directory {
            let mut dentries = self.dentries.write();
            dentries.remove(&(target_inode_no, ".".to_string()));
            dentries.remove(&(target_inode_no, "..".to_string()));
        }
        // Remove inode
        self.inodes.write().remove(&target_inode_no);

        Ok(())
    }

    fn free_inode(&self, inode_no: InodeNo) -> Result<(), Error> {
        // Check mount state
        self.check_mounted()?;
        // Check if inode exists
        if !self.inodes.read().contains_key(&inode_no) {
            return Err(code::ENOENT);
        }
        // Remove from inode table
        self.inodes.write().remove(&inode_no);
        // Remove all related entries from directory entry table
        self.dentries.write().retain(|_, &mut v| v != inode_no);

        Ok(())
    }

    fn sync(&self) -> Result<(), Error> {
        Err(code::EAGAIN)
    }
}

impl Default for TmpFileSystem {
    fn default() -> Self {
        Self::new()
    }
}
