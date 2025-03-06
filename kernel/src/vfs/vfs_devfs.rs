#![allow(dead_code)]

use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    vec::Vec,
};
use core::{
    ffi::{c_char, c_int, c_void},
    sync::atomic::{AtomicPtr, Ordering},
};
use spin::RwLock as SpinRwLock;

use crate::{
    error::{code, Error},
    vfs::{
        vfs_dirent::*,
        vfs_log::*,
        vfs_mode::*,
        vfs_node::{FileType, InodeAttr, InodeNo},
        vfs_traits::{FileOperationTrait, FileSystemTrait},
    },
};

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub enum DevType {
    Char = 0,
    Block = 1,
    Other = 255,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct DevInfo {
    name: [c_char; 8], // RT_NAME_MAX
    type_: DevType,
    ref_count: u16,
}

extern "C" {
    fn dev_manager_create() -> *mut c_void;
    fn dev_manager_destroy(manager: *mut c_void);
    fn dev_manager_get_count(manager: *mut c_void) -> c_int;
    fn dev_manager_get_devices(
        manager: *mut c_void,
        devices: *mut DevInfo,
        max_count: c_int,
    ) -> c_int;
    fn dev_manager_update(manager: *mut c_void) -> c_int;
    fn dev_manager_open(manager: *mut c_void, name: *const c_char, flags: c_int) -> *mut c_void;
    fn dev_manager_close(manager: *mut c_void, dev: *mut c_void) -> c_int;
    fn dev_manager_read(
        manager: *mut c_void,
        dev: *mut c_void,
        pos: u32,
        buffer: *mut c_void,
        size: usize,
    ) -> c_int;
    fn dev_manager_write(
        manager: *mut c_void,
        dev: *mut c_void,
        pos: u32,
        buffer: *const c_void,
        size: usize,
    ) -> c_int;
    fn dev_manager_control(
        manager: *mut c_void,
        dev: *mut c_void,
        cmd: c_int,
        args: *mut c_void,
    ) -> c_int;
}

#[derive(Debug)]
struct SafeDevHandle(AtomicPtr<c_void>);

unsafe impl Send for SafeDevHandle {}
unsafe impl Sync for SafeDevHandle {}

impl SafeDevHandle {
    fn new(ptr: *mut c_void) -> Self {
        Self(AtomicPtr::new(ptr))
    }

    fn get(&self) -> *mut c_void {
        self.0.load(Ordering::Acquire)
    }

    fn set(&self, ptr: *mut c_void) {
        self.0.store(ptr, Ordering::Release)
    }

    fn is_null(&self) -> bool {
        self.get().is_null()
    }
}

impl Default for SafeDevHandle {
    fn default() -> Self {
        Self::new(core::ptr::null_mut())
    }
}

#[derive(Debug)]
struct DevNode {
    attr: InodeAttr,
    dev_handle: SafeDevHandle,
    name: String,
    offset: usize,
}

#[cfg_attr(feature = "cbindgen", no_mangle)]
pub struct DevFileSystem {
    mounted: SpinRwLock<bool>,
    mount_point: SpinRwLock<String>,
    dev_nodes: SpinRwLock<BTreeMap<InodeNo, DevNode>>,
    next_inode_no: SpinRwLock<InodeNo>,
    manager: SpinRwLock<SafeDevHandle>,
}

impl DevFileSystem {
    pub fn new() -> Self {
        vfslog!("[devfs] Creating new DevFS instance");

        DevFileSystem {
            mounted: SpinRwLock::new(false),
            mount_point: SpinRwLock::new(String::new()),
            dev_nodes: SpinRwLock::new(BTreeMap::new()),
            next_inode_no: SpinRwLock::new(1),
            manager: SpinRwLock::new(SafeDevHandle::default()),
        }
    }

    fn check_mounted(&self) -> Result<(), Error> {
        if !*self.mounted.read() {
            Err(code::EAGAIN)
        } else {
            Ok(())
        }
    }

    fn alloc_inode_no(&self) -> InodeNo {
        let mut next_inode_no = self.next_inode_no.write();
        let inode_no = *next_inode_no;
        *next_inode_no += 1;
        inode_no
    }

    fn scan_devices(&self) -> i32 {
        let manager = self.manager.read();
        let manager_ptr = manager.get();
        if manager_ptr.is_null() {
            return -1;
        }

        let count = unsafe { dev_manager_get_count(manager_ptr) };
        if count <= 0 {
            return count;
        }

        let mut devices = Vec::<DevInfo>::with_capacity(count as usize);
        unsafe { devices.set_len(count as usize) };

        let ret = unsafe { dev_manager_get_devices(manager_ptr, devices.as_mut_ptr(), count) };
        vfslog!("[devfs] Retrieved {} device entries", ret);
        if ret > 0 {
            for dev in devices {
                self.add_device(&dev);
            }
        }

        ret
    }

    fn add_device(&self, dev_info: &DevInfo) {
        let inode_no = self.alloc_inode_no();

        let attr = InodeAttr {
            inode_no,
            size: 0,
            blocks: 0,
            atime: 0,
            mtime: 0,
            ctime: 0,
            file_type: match dev_info.type_ {
                DevType::Char => FileType::CharDevice,
                DevType::Block => FileType::BlockDevice,
                _ => FileType::CharDevice,
            },
            mode: 0o666 | S_IFCHR,
            nlinks: 1,
            uid: 0,
            gid: 0,
        };

        // Convert C char array to Rust string
        let name = unsafe {
            let name_slice = core::slice::from_raw_parts(
                dev_info.name.as_ptr() as *const u8,
                dev_info.name.iter().position(|&c| c == 0).unwrap_or(8),
            );
            String::from_utf8_lossy(name_slice).into_owned()
        };

        let node = DevNode {
            attr,
            dev_handle: SafeDevHandle::default(),
            name,
            offset: 0,
        };

        self.dev_nodes.write().insert(inode_no, node);
    }
}

impl FileOperationTrait for DevFileSystem {
    fn open(&self, path: &str, flags: i32) -> Result<InodeNo, Error> {
        vfslog!("[DevFS] Opening device: {} (flags: {})", path, flags);

        self.check_mounted()?;
        // Update device node list
        vfslog!("[DevFS] Updating device list before opening");

        let manager = self.manager.read();
        if !manager.is_null() {
            let update_result = unsafe { dev_manager_update(manager.get()) };
            vfslog!("[DevFS] Device list update result: {}", update_result);

            let scan_result = self.scan_devices();
            vfslog!("[DevFS] Device scan result: {}", scan_result);
        }

        let dev_name = path.trim_start_matches('/');
        let manager = self.manager.read();

        let mut nodes = self.dev_nodes.write();
        let node_entry = nodes.iter_mut().find(|(_, node)| node.name == dev_name);

        match node_entry {
            Some((inode_no, node)) => {
                let name_cstr = alloc::ffi::CString::new(dev_name).unwrap();
                vfslog!("[DevFS] Attempting to open device: {}", dev_name);
                let handle = unsafe { dev_manager_open(manager.get(), name_cstr.as_ptr(), flags) };

                if handle.is_null() {
                    vfslog!("[DevFS] Failed to open device: {}", dev_name);
                    return Err(code::ENOENT);
                }

                node.dev_handle.set(handle);
                vfslog!("[DevFS] Device opened successfully: {}", dev_name);
                Ok(*inode_no)
            }
            None => {
                vfslog!("[DevFS] Device node not found: {}", dev_name);
                Err(code::ENOENT)
            }
        }
    }

    fn read(&self, inode_no: InodeNo, buf: &mut [u8], offset: &mut usize) -> Result<usize, Error> {
        self.check_mounted()?;

        let manager = self.manager.read();
        let nodes = self.dev_nodes.read();

        let node = match nodes.get(&inode_no) {
            Some(node) => node,
            None => return Err(code::ENOENT),
        };

        let dev_handle = node.dev_handle.get();
        if dev_handle.is_null() {
            return Err(code::EINVAL);
        }

        let ret = unsafe {
            dev_manager_read(
                manager.get(),
                dev_handle,
                *offset as u32,
                buf.as_mut_ptr() as *mut c_void,
                buf.len(),
            )
        };

        if ret >= 0 {
            *offset += ret as usize;
            Ok(ret as usize)
        } else {
            Ok(ret as usize)
        }
    }

    fn write(&self, inode_no: InodeNo, buf: &[u8], offset: &mut usize) -> Result<usize, Error> {
        vfslog!(
            "[devfs] Writing {} bytes at offset {} for inode {}",
            buf.len(),
            offset,
            inode_no
        );

        self.check_mounted()?;

        let manager = self.manager.read();
        let nodes = self.dev_nodes.read();

        let node = match nodes.get(&inode_no) {
            Some(node) => node,
            None => return Err(code::ENOENT),
        };

        let dev_handle = node.dev_handle.get();
        if dev_handle.is_null() {
            return Err(code::EINVAL);
        }

        let ret = unsafe {
            dev_manager_write(
                manager.get(),
                dev_handle,
                *offset as u32,
                buf.as_ptr() as *const c_void,
                buf.len(),
            )
        };

        if ret >= 0 {
            vfslog!("[devfs] Successfully wrote {} bytes", ret);
            *offset += ret as usize;
            Ok(ret as usize)
        } else {
            vfslog!("[devfs] Write failed with error: {}", ret);
            Err(Error::from_errno(ret))
        }
    }

    fn get_offset(&self, inode_no: InodeNo) -> Result<usize, Error> {
        self.check_mounted()?;

        let nodes = self.dev_nodes.read();
        match nodes.get(&inode_no) {
            Some(node) => Ok(node.offset),
            None => Err(code::ENOENT),
        }
    }

    fn seek(&self, inode_no: InodeNo, offset: usize, whence: i32) -> Result<usize, Error> {
        self.check_mounted()?;

        let mut nodes = self.dev_nodes.write();
        let node = match nodes.get_mut(&inode_no) {
            Some(node) => node,
            None => return Err(code::ENOENT),
        };

        // Calculate new offset based on whence
        let new_offset = match whence {
            SEEK_SET => offset,
            SEEK_CUR => node.offset.saturating_add(offset),
            SEEK_END => {
                // Usually SEEK_END is not supported for device files
                return Err(code::EINVAL);
            }
            _ => return Err(code::EINVAL),
        };

        node.offset = new_offset;
        Ok(new_offset)
    }

    fn size(&self, inode_no: InodeNo) -> Result<usize, Error> {
        self.check_mounted()?;

        let nodes = self.dev_nodes.read();
        match nodes.get(&inode_no) {
            Some(node) => match node.attr.file_type {
                FileType::BlockDevice => Ok(node.attr.size),
                _ => Ok(0),
            },
            None => Err(code::ENOENT),
        }
    }

    fn flush(&self, inode_no: InodeNo) -> Result<(), Error> {
        self.check_mounted()?;
        let nodes = self.dev_nodes.read();
        match nodes.get(&inode_no) {
            Some(_) => Ok(()),
            None => Err(code::ENOENT),
        }
    }

    fn fsync(&self, inode_no: InodeNo) -> Result<(), Error> {
        self.check_mounted()?;
        let nodes = self.dev_nodes.read();
        match nodes.get(&inode_no) {
            Some(_) => Ok(()),
            None => Err(code::ENOENT),
        }
    }

    fn truncate(&self, inode_no: InodeNo, _size: usize) -> Result<(), Error> {
        self.check_mounted()?;

        let nodes = self.dev_nodes.read();
        match nodes.get(&inode_no) {
            Some(_) => Err(code::EAGAIN), // devfs not support truncate
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
        vfslog!(
            "[devfs] getdents: inode_no={}, offset={}, count={}",
            inode_no,
            offset,
            count
        );

        // Check if filesystem is mounted
        self.check_mounted()?;
        let nodes = self.dev_nodes.read();
        let dir_node = match nodes.get(&inode_no) {
            Some(node) => node,
            None => {
                vfslog!("[devfs] getdents: directory node not found");
                return Err(code::ENOENT);
            }
        };

        // Check if it's a directory
        if dir_node.attr.file_type != FileType::Directory {
            vfslog!("[devfs] getdents: not a directory");
            return Err(code::ENOTDIR);
        }

        // Collect and sort device nodes
        let mut entries: Vec<_> = nodes
            .iter()
            .map(|(_, node)| {
                // Create directory entry
                let d_type = match node.attr.file_type {
                    FileType::Regular => DT_REG,
                    FileType::Directory => DT_DIR,
                    FileType::SymLink => DT_LNK,
                    FileType::CharDevice => DT_CHR,
                    FileType::BlockDevice => DT_BLK,
                };

                Dirent::new(d_type, node.name.clone())
            })
            .collect();
        // Sort by name
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        // Check offset
        let start_idx = offset as usize;
        if start_idx >= entries.len() {
            vfslog!("[devfs] getdents: offset beyond end of entries");
            return Ok(0);
        }

        // Clear input vector
        dirents.clear();
        // Get number of entries to return
        let entries_to_write = core::cmp::min(count, entries.len() - start_idx);
        // Add directory entries to output vector
        dirents.extend(entries.into_iter().skip(start_idx).take(entries_to_write));
        vfslog!("[devfs] getdents: returned {} entries", entries_to_write);

        Ok(entries_to_write)
    }
}

impl FileSystemTrait for DevFileSystem {
    fn mount(
        &self,
        _source: &str,
        target: &str,
        _flags: u64,
        _data: Option<&[u8]>,
    ) -> Result<(), Error> {
        if *self.mounted.read() {
            return Err(code::EEXIST);
        }

        unsafe {
            let manager = dev_manager_create();
            if manager.is_null() {
                return Err(code::EAGAIN);
            }

            self.manager.write().set(manager);
            *self.mount_point.write() = target.to_string();
            *self.mounted.write() = true;

            self.scan_devices();
        }

        Ok(())
    }

    fn unmount(&self, target: &str) -> Result<(), Error> {
        if !*self.mounted.read() {
            return Err(code::EAGAIN);
        }

        if target != *self.mount_point.read() {
            return Err(code::EINVAL);
        }

        unsafe {
            let manager = self.manager.read();
            if !manager.is_null() {
                dev_manager_destroy(manager.get());
            }
        }

        self.manager.write().set(core::ptr::null_mut());
        *self.mounted.write() = false;
        self.mount_point.write().clear();
        self.dev_nodes.write().clear();
        *self.next_inode_no.write() = 2;

        Ok(())
    }

    fn create_inode(&self, _path: &str, _mode: u32) -> Result<InodeAttr, Error> {
        Err(code::EAGAIN)
    }

    fn remove_inode(&self, _path: &str) -> Result<(), Error> {
        Err(code::EAGAIN)
    }

    fn free_inode(&self, _inode_no: InodeNo) -> Result<(), Error> {
        Err(code::EAGAIN)
    }

    fn sync(&self) -> Result<(), Error> {
        Err(code::EAGAIN)
    }

    // fn lookup_inode(&self, inode_no: InodeNo) -> Result<InodeAttr, i32> {
    //     if self.check_mounted() != SUCCESS {
    //         return Err(code::EAGAIN);
    //     }

    //     self.dev_nodes
    //         .read()
    //         .get(&inode_no)
    //         .map(|node| Ok(node.attr.clone()))
    //         .unwrap_or(Err(ENOENT))
    // }
}

impl Drop for DevFileSystem {
    fn drop(&mut self) {
        unsafe {
            let manager = self.manager.read();
            if !manager.is_null() {
                dev_manager_destroy(manager.get());
            }
        }
    }
}
