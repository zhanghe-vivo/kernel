//! vfs_procfs.rs  
use crate::{
    error::{code, Error},
    process::{foreach, Kprocess},
    thread::Thread,
    vfs::{
        procfs::*,
        vfs_dirent::*,
        vfs_manager::get_vfs_manager,
        vfs_mode::*,
        vfs_node::{FileType, InodeAttr, InodeNo},
        vfs_path::*,
        vfs_traits::{FileOperationTrait, FileSystemTrait},
    },
};
use alloc::{
    borrow::ToOwned,
    collections::BTreeMap,
    format,
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use bluekernel_infra::list::doubly_linked_list::{LinkedListNode, ListHead};
use core::{
    cmp::min,
    sync::atomic::{AtomicBool, Ordering},
};
use libc::{O_APPEND, O_DIRECTORY, O_RDWR, O_TRUNC, O_WRONLY, S_IFDIR, S_IFLNK, S_IFMT, S_IFREG};
use log::{info, warn};
use spin::RwLock as SpinRwLock;

// File access permissions
const R_OK: u32 = 4; // Read permission
const W_OK: u32 = 2; // Write permission
const X_OK: u32 = 1; // Execute permission

const DEAULT_PERMISSION: u32 = 0o555;

pub trait ProcNodeOperationTrait: Send + Sync {
    // Obtain the file content when a read operation is performed on a proc inode.
    fn get_content(&self) -> Result<Vec<u8>, Error>;
    // Set the file content when a write operation is performed on a proc inode.
    fn set_content(&self, content: Vec<u8>) -> Result<(usize), Error>;
}

struct DefaultProcNodeOperationTrait;

impl ProcNodeOperationTrait for DefaultProcNodeOperationTrait {
    fn get_content(&self) -> Result<Vec<u8>, Error> {
        Ok(Vec::new())
    }

    fn set_content(&self, content: Vec<u8>) -> Result<(usize), Error> {
        Ok(0)
    }
}

/// Inode in proc filesystem
struct ProcInode {
    attr: InodeAttr,
    proc_node_op: Option<Arc<dyn ProcNodeOperationTrait>>,
    data: Vec<u8>,
    read_pos: usize, // The offset of the last read
}

impl ProcInode {
    fn new(attr: InodeAttr, proc_node_op: Option<Arc<dyn ProcNodeOperationTrait>>) -> Self {
        Self {
            attr,
            proc_node_op,
            data: Vec::new(),
            read_pos: 0,
        }
    }
}

static INITED: AtomicBool = AtomicBool::new(false);
/// Proc filesystem implementation
#[cfg_attr(feature = "cbindgen", no_mangle)]
pub struct ProcFileSystem {
    mounted: AtomicBool,
    mount_point: SpinRwLock<String>,
    // Inode table
    inodes: SpinRwLock<BTreeMap<InodeNo, ProcInode>>,
    // Directory entry table (parent inode number, filename) -> child inode number
    dentries: SpinRwLock<BTreeMap<(InodeNo, String), InodeNo>>,
    // Next available inode number
    next_inode_no: SpinRwLock<InodeNo>,
}

impl ProcFileSystem {
    pub fn new() -> Self {
        Self {
            mounted: AtomicBool::new(false),
            mount_point: SpinRwLock::new(String::new()),
            inodes: SpinRwLock::new(BTreeMap::new()),
            dentries: SpinRwLock::new(BTreeMap::new()),
            next_inode_no: SpinRwLock::new(1), // Start from 1, will be used for root directory
        }
    }

    pub fn init() -> Result<(), Error> {
        INITED.store(true, Ordering::Relaxed);
        Self::proc_create_file(
            "/proc/meminfo",
            DEAULT_PERMISSION,
            Some(Arc::new(ProcMemoryInfoFileOp::new())),
        )?;
        Self::proc_create_file(
            "/proc/stat",
            DEAULT_PERMISSION,
            Some(Arc::new(ProcStatFileOp::new())),
        )?;

        let process = Kprocess::get_process();
        Self::proc_mkdir(format!("/proc/{}", process.pid).as_str(), DEAULT_PERMISSION)?;
        Self::proc_mkdir(
            format!("/proc/{}/task", process.pid).as_str(),
            DEAULT_PERMISSION,
        )?;
        crate::foreach!(
            node,
            list,
            crate::object::ObjectClassType::ObjectClassThread,
            {
                unsafe {
                    let kobject =
                        crate::list_head_entry!(node.as_ptr(), crate::object::KObjectBase, list);
                    let thread: *const Thread =
                        crate::list_head_entry!(kobject, crate::thread::Thread, parent);
                    let thread_name = (*thread).get_name().to_str().expect("CStr to str failed");
                    let dir_path = format!("/proc/{}/task/{}", process.pid, (*thread).tid);
                    match ProcFileSystem::proc_mkdir(&dir_path, DEAULT_PERMISSION) {
                        Err(err) => return Err(err),
                        Ok(_) => {
                            let stat_path = format!("{}/status", dir_path);
                            ProcFileSystem::proc_create_file(
                                &stat_path,
                                DEAULT_PERMISSION,
                                Some(Arc::new(ProcTaskFileOp::new(
                                    process.pid,
                                    (*thread).tid,
                                    thread_name,
                                ))),
                            )?;
                        }
                    };
                }
            }
        );
        Ok(())
    }

    pub fn proc_create_file(
        path: &str,
        mode: u32,
        proc_node_op: Option<Arc<dyn ProcNodeOperationTrait>>,
    ) -> Result<(), Error> {
        Self::proc_create(path, mode | S_IFREG, proc_node_op)
    }

    pub fn proc_create_symlink(path: &str, dest: &str) -> Result<(), Error> {
        Self::proc_create(path, 0o777 | S_IFLNK, None)
    }

    // make a dir
    pub fn proc_mkdir(path: &str, mode: u32) -> Result<(), Error> {
        Self::proc_create(path, mode | S_IFDIR, None)
    }

    fn proc_create(
        path: &str,
        mode: u32,
        proc_node_op: Option<Arc<dyn ProcNodeOperationTrait>>,
    ) -> Result<(), Error> {
        let vfs_manager = get_vfs_manager();
        if let Some(fs) = vfs_manager.get_fs("procfs") {
            let proc_fs: &ProcFileSystem = fs
                .as_any()
                .downcast_ref::<ProcFileSystem>()
                .expect("cannot downcast_ref to ProcFileSystem");
            match proc_fs.create_internal(path, mode, proc_node_op) {
                Ok(_) => Ok(()),
                Err(e) => Err(e),
            }
        } else {
            Err(code::EAGAIN)
        }
    }

    pub fn proc_remove(path: &str) -> Result<(), Error> {
        let vfs_manager = get_vfs_manager();
        if let Some(fs) = vfs_manager.get_fs("procfs") {
            let proc_fs: &ProcFileSystem = fs
                .as_any()
                .downcast_ref::<ProcFileSystem>()
                .expect("cannot downcast_ref to ProcFileSystem");
            match proc_fs.remove_internal(path) {
                Ok(_) => Ok(()),
                Err(e) => Err(e),
            }
        } else {
            Err(code::EAGAIN)
        }
    }

    pub fn proc_find(path: &str) -> Result<(InodeAttr), Error> {
        let vfs_manager = get_vfs_manager();
        if let Some(fs) = vfs_manager.get_fs("procfs") {
            let proc_fs: &ProcFileSystem = fs
                .as_any()
                .downcast_ref::<ProcFileSystem>()
                .expect("cannot downcast_ref to ProcFileSystem");
            proc_fs.find_internal(path)
        } else {
            Err(code::EAGAIN)
        }
    }

    fn create_internal(
        &self,
        path: &str,
        mode: u32,
        proc_node_op: Option<Arc<dyn ProcNodeOperationTrait>>,
    ) -> Result<InodeNo, Error> {
        Self::check_inited()?;
        self.check_mounted()?;
        if !is_valid_path(path) {
            warn!("[procfs] create_internal: invalid path {}", path);
            return Err(code::EINVAL);
        }
        // Normalize path
        let mut path = normalize_path(path).ok_or(code::EINVAL)?;
        // Check if path is under mount point
        if !path.starts_with(&*(self.mount_point.read())) {
            return Err(code::EINVAL);
        }
        // TODO: Using relative paths
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
        let inode_no = self.alloc_inode_no();
        let file_type = if mode & S_IFMT == S_IFDIR {
            FileType::Directory
        } else if mode & S_IFMT == S_IFREG {
            FileType::Regular
        } else if mode & S_IFMT == S_IFLNK {
            FileType::SymLink
        } else {
            FileType::Regular // Default to regular file
        };
        let attr = InodeAttr::new(inode_no, file_type, mode);
        // Add inode and directory entry
        self.inodes
            .write()
            .insert(inode_no, ProcInode::new(attr, proc_node_op));
        self.dentries
            .write()
            .insert((parent_inode_no, name.to_string()), inode_no);
        // If directory, add . and .. entries
        if file_type == FileType::Directory {
            let mut dentries = self.dentries.write();
            dentries.insert((inode_no, ".".to_string()), inode_no);
            dentries.insert((inode_no, "..".to_string()), parent_inode_no);
        }
        Ok(inode_no)
    }

    fn remove_internal(&self, path: &str) -> Result<(), Error> {
        Self::check_inited()?;
        let param_path = path;
        self.check_mounted()?;
        if !is_valid_path(path) {
            return Err(code::EINVAL);
        }
        // Normalize path
        let path = normalize_path(path).ok_or(code::EINVAL)?;
        // Check if path is under mount point
        if !path.starts_with(&*(self.mount_point.read())) {
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
        // If directory, remomve child nodes first.
        if is_directory {
            let children: Vec<String> = self
                .dentries
                .read()
                .iter()
                .filter(|((parent_inode, name), child_inode)| {
                    *parent_inode == target_inode_no && name != "." && name != ".."
                })
                .map(|((parent_inode, name), child_inode)| path.clone() + "/" + name) // 克隆 String 以获取所有权
                .collect();
            for child_path in children {
                self.remove_internal(&child_path)?;
            }
            // remove . and .. entries
            let mut dentries = self.dentries.write();
            dentries.remove(&(target_inode_no, ".".to_string()));
            dentries.remove(&(target_inode_no, "..".to_string()));
        }
        // Remove directory entry
        self.dentries
            .write()
            .remove(&(parent_inode_no, name.to_string()));
        // Remove inode
        self.inodes.write().remove(&target_inode_no);
        Ok(())
    }

    fn find_internal(&self, path: &str) -> Result<(InodeAttr), Error> {
        Self::check_inited()?;
        let inode_no = self.lookup_path(path)?;
        let inodes = self.inodes.read();
        let inode: &ProcInode = inodes.get(&inode_no).ok_or(code::ENOENT)?;
        Ok(inode.attr.clone())
    }

    fn check_mounted(&self) -> Result<(), Error> {
        if !self.mounted.load(Ordering::Relaxed) {
            return Err(code::EAGAIN);
        }
        Ok(())
    }

    fn check_inited() -> Result<(), Error> {
        if !INITED.load(Ordering::Relaxed) {
            return Err(code::EAGAIN);
        }
        Ok(())
    }

    // Allocate new inode number
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

    /// Look up parent directory inode and filename
    fn lookup_parent(&self, path: &str) -> Result<(InodeNo, String), Error> {
        // Split path
        let (parent_path, name) = split_path(path).ok_or(code::EINVAL)?;

        // Find parent directory inode
        let parent_inode_no = self.lookup_path(parent_path)?;

        Ok((parent_inode_no, name.to_string()))
    }

    pub(crate) fn trace_thread_init(tid: usize, name: &str) -> Result<(), Error> {
        let pid = Kprocess::get_process().pid;
        let dir_path = format!("/proc/{}/task/{}", pid, tid);
        ProcFileSystem::proc_mkdir(&dir_path, DEAULT_PERMISSION)?;
        let stat_path = format!("{}/status", dir_path);
        ProcFileSystem::proc_create_file(
            &stat_path,
            DEAULT_PERMISSION,
            Some(Arc::new(ProcTaskFileOp::new(pid, tid, name))),
        )?;
        Ok(())
    }

    pub(crate) fn trace_thread_close(tid: usize) -> Result<(), Error> {
        let path = format!("/proc/{}/task/{}", Kprocess::get_process().pid, tid);
        if let Ok(_) = ProcFileSystem::proc_find(&path) {
            let _ = ProcFileSystem::proc_remove(&path)?;
        }
        Ok(())
    }
}

impl FileOperationTrait for ProcFileSystem {
    fn open(&self, path: &str, flags: i32) -> Result<InodeNo, Error> {
        self.check_mounted()?;
        let inode_no = self.path_to_inode(path)?;
        let mut inodes = self.inodes.write();
        let inode: &ProcInode = match inodes.get_mut(&inode_no) {
            Some(inode) => inode,
            None => return Err(code::ENOENT),
        };
        // Handle O_DIRECTORY flag
        if (flags & O_DIRECTORY) != 0 {
            // If O_DIRECTORY is specified but the path is not a directory, return error
            if inode.attr.file_type != FileType::Directory {
                warn!("[procfs] open: O_DIRECTORY specified but path is not a directory");
                return Err(code::ENOTDIR);
            }
        } else if inode.attr.file_type == FileType::Directory {
            // If opening a directory but not specifying O_DIRECTORY, also return error
            warn!("[procfs] open: opening directory without O_DIRECTORY flag");
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
        if inode.attr.mode & access_mode != access_mode {
            return Err(code::EACCES);
        }
        Ok(inode_no)
    }

    fn close(&self, inode_no: InodeNo) -> Result<(), Error> {
        self.check_mounted()?;
        let mut inodes = self.inodes.write();
        match inodes.get_mut(&inode_no) {
            Some(inode) => inode,
            None => return Err(code::ENOENT),
        };
        Ok(())
    }

    fn read(&self, inode_no: InodeNo, buf: &mut [u8], offset: &mut usize) -> Result<usize, Error> {
        self.check_mounted()?;
        let mut inodes = self.inodes.write();
        let inode: &mut ProcInode = match inodes.get_mut(&inode_no) {
            Some(inode) => inode,
            None => return Err(code::ENOENT),
        };
        let mut read_size = 0;
        let origin_offset = *offset;
        if origin_offset == 0 {
            // The first read with offset 0
            inode.data.clear();
            inode.read_pos = 0;
            // get the latest content
            if let Some(proc_node_op) = &inode.proc_node_op {
                inode.data = proc_node_op.get_content()?;
            }
        }
        let available = (inode.data.len()).saturating_sub(origin_offset);
        read_size = min(available, buf.len());
        let start = origin_offset;
        let end = start + read_size;

        buf[..read_size].copy_from_slice(&inode.data[start..end]);

        // Update input offset
        *offset += read_size;
        inode.read_pos = *offset;
        Ok(read_size)
    }

    fn write(&self, inode_no: InodeNo, buf: &[u8], offset: &mut usize) -> Result<usize, Error> {
        self.check_mounted()?;
        let mut inodes = self.inodes.write();
        let inode: &mut ProcInode = match inodes.get_mut(&inode_no) {
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

        if let Some(proc_node_op) = &inode.proc_node_op {
            proc_node_op.set_content(Vec::from(buf))?;
        }
        Ok(buf.len())
    }

    fn get_offset(&self, inode_no: InodeNo) -> Result<usize, Error> {
        self.check_mounted()?;
        let inodes = self.inodes.read();
        match inodes.get(&inode_no) {
            Some(_) => Err(code::ENOSYS),
            None => Err(code::ENOENT),
        }
    }

    fn seek(&self, inode_no: InodeNo, offset: usize, whence: i32) -> Result<usize, Error> {
        self.check_mounted()?;
        let inodes = self.inodes.read();
        match inodes.get(&inode_no) {
            Some(_) => Err(code::ENOSYS),
            None => Err(code::ENOENT),
        }
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
            Some(_) => Err(code::ENOSYS),
            None => Err(code::ENOENT),
        }
    }

    fn fsync(&self, inode_no: InodeNo) -> Result<(), Error> {
        self.check_mounted()?;
        let inodes = self.inodes.read();
        match inodes.get(&inode_no) {
            Some(_) => Err(code::ENOSYS),
            None => Err(code::ENOENT),
        }
    }

    fn truncate(&self, inode_no: InodeNo, size: usize) -> Result<(), Error> {
        self.check_mounted()?;
        let mut inodes = self.inodes.write();
        match inodes.get_mut(&inode_no) {
            Some(_) => Err(code::ENOSYS),
            None => Err(code::ENOENT),
        }
    }

    fn getdents(
        &self,
        inode_no: InodeNo,
        offset: usize,
        dirents: &mut Vec<Dirent>,
        count: usize,
    ) -> Result<usize, Error> {
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
                        FileType::Unknown => DT_UNKNOWN,
                        FileType::Regular => DT_REG,
                        FileType::Directory => DT_DIR,
                        FileType::SymLink => DT_LNK,
                        FileType::CharDevice => DT_CHR,
                        FileType::BlockDevice => DT_BLK,
                        FileType::Fifo => DT_FIFO,
                        FileType::Socket => DT_SOCK,
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
        Ok(entries_to_write)
    }
}

impl FileSystemTrait for ProcFileSystem {
    fn mount(
        &self,
        _source: &str,
        target: &str,
        _flags: u64,
        _data: Option<&[u8]>,
    ) -> Result<(), Error> {
        // Check if already mounted
        if self.mounted.load(Ordering::Relaxed) {
            warn!("[procfs] Already mounted");
            return Err(code::EEXIST);
        }
        // Check mount point path
        if target.is_empty() {
            warn!("[procfs] Empty mount point");
            return Err(code::EINVAL);
        }
        // Normalize mount point path
        let normalized_target = match normalize_path(target) {
            Some(path) => path,
            None => {
                warn!("[procfs] Invalid mount point path {}", target);
                return Err(code::EINVAL);
            }
        };
        // Allocate root directory inode number
        let root_inode_no = self.alloc_inode_no();
        // Create root directory inode
        let root_attr = InodeAttr::new(root_inode_no, FileType::Directory, 0o755 | S_IFDIR);
        // Initialize filesystem state
        {
            let mut inodes = self.inodes.write();
            let mut dentries = self.dentries.write();
            // Clear existing data
            inodes.clear();
            dentries.clear();
            // Insert root directory inode
            inodes.insert(root_inode_no, ProcInode::new(root_attr, None));
            // Add . and .. entries for root directory
            dentries.insert((root_inode_no, ".".to_string()), root_inode_no);
            dentries.insert((root_inode_no, "..".to_string()), root_inode_no);
        }
        // Set mount point and state
        let mount_path = normalized_target.clone(); // Clone for log output

        *self.mount_point.write() = normalized_target;
        self.mounted.store(true, Ordering::Relaxed);

        let mount_point = self.mount_point.read();
        let mount_point: String = mount_point.clone();

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
        // Operating proc inode via posix is not supported.
        Err(code::ENOSYS)
    }

    fn remove_inode(&self, path: &str) -> Result<(), Error> {
        // Operating proc inode via posix is not supported.
        Err(code::ENOSYS)
    }

    fn free_inode(&self, inode_no: InodeNo) -> Result<(), Error> {
        // Operating proc inode via posix is not supported.
        Err(code::ENOSYS)
    }

    fn sync(&self) -> Result<(), Error> {
        Err(code::ENOSYS)
    }

    fn lookup_path(&self, path: &str) -> Result<InodeNo, Error> {
        if !is_valid_path(path) {
            warn!("[procfs] lookup_path: Invalid path: {}", path);
            return Err(code::EINVAL);
        }
        // Normalize path
        let path = normalize_path(path).ok_or(code::EINVAL)?;
        // Get mount point
        let mount_point = self.mount_point.read();

        if mount_point.is_empty() {
            warn!("[procfs] lookup_path: Filesystem not mounted");
            return Err(code::EINVAL);
        }
        // Check if path is under mount point
        if !path.starts_with(&*mount_point) {
            warn!(
                "[procfs] lookup_path: Path {} not under mount point {}",
                path, *mount_point
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
                    return Err(code::ENOENT);
                }
            }
        }
        Ok(current_inode_no)
    }
}

impl Default for ProcFileSystem {
    fn default() -> Self {
        Self::new()
    }
}
