#![allow(dead_code)]

use crate::drivers::device::{Device, DeviceClass, DeviceManager};

use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use core::sync::atomic::{AtomicBool, Ordering};
use libc::{SEEK_CUR, SEEK_END, SEEK_SET, S_IFCHR};
use spin::RwLock as SpinRwLock;

use crate::{
    error::{code, Error},
    vfs::{
        vfs_dirent::*,
        vfs_log::*,
        vfs_node::{FileType, InodeAttr, InodeNo},
        vfs_traits::{FileOperationTrait, FileSystemTrait},
    },
};

struct DevNode {
    attr: InodeAttr,
    offset: usize,
    dev: Arc<dyn Device>,
}

pub struct DevFileSystem {
    mounted: AtomicBool,
    mount_point: SpinRwLock<String>,
    dev_nodes: SpinRwLock<BTreeMap<InodeNo, DevNode>>,
    next_inode_no: SpinRwLock<InodeNo>,
    manager: &'static DeviceManager,
}

impl DevFileSystem {
    pub fn new(manager: &'static DeviceManager) -> Self {
        vfslog!("[devfs] Creating new DevFS instance");

        DevFileSystem {
            mounted: AtomicBool::new(false),
            mount_point: SpinRwLock::new(String::new()),
            dev_nodes: SpinRwLock::new(BTreeMap::new()),
            next_inode_no: SpinRwLock::new(1),
            manager,
        }
    }

    fn check_mounted(&self) -> Result<(), Error> {
        if !self.mounted.load(Ordering::Relaxed) {
            return Err(code::ENOENT);
        }

        Ok(())
    }

    fn alloc_inode_no(&self) -> InodeNo {
        let mut next_inode_no = self.next_inode_no.write();
        let inode_no = *next_inode_no;
        *next_inode_no += 1;
        inode_no
    }

    fn scan_devices(&self) -> Result<(), Error> {
        self.manager
            .foreach(|dev| {
                vfslog!("[devfs] Adding device: {}", dev.name());
                self.add_device(dev);
            })
            .map_err(|e| Error::from_errno(e as i32))
    }

    fn add_device(&self, dev: Arc<dyn Device>) {
        let inode_no = self.alloc_inode_no();

        let attr = InodeAttr {
            inode_no,
            size: 0,
            blocks: 0,
            atime: 0,
            mtime: 0,
            ctime: 0,
            file_type: match dev.class() {
                DeviceClass::Char => FileType::CharDevice,
                DeviceClass::Block => FileType::BlockDevice,
            },
            mode: u32::from(dev.access_mode()) | S_IFCHR,
            nlinks: 1,
            uid: 0,
            gid: 0,
        };

        let node = DevNode {
            attr,
            offset: 0,
            dev: dev.clone(),
        };

        self.dev_nodes.write().insert(inode_no, node);

        vfslog!(
            "[devfs] Added device: {} (inode_no: {})",
            dev.name(),
            inode_no
        );
    }
}

impl FileOperationTrait for DevFileSystem {
    fn open(&self, path: &str, flags: i32) -> Result<InodeNo, Error> {
        vfslog!("[DevFS] Opening device: {} (flags: {})", path, flags);

        self.check_mounted()?;

        let dev_name = path.trim_start_matches('/');
        let mut nodes = self.dev_nodes.write();
        let node_entry = nodes
            .iter_mut()
            .find(|(_, node)| node.dev.name() == dev_name);

        match node_entry {
            Some((inode_no, node)) => {
                vfslog!("[DevFS] Attempting to open device: {}", dev_name);

                match node.dev.open(flags) {
                    Ok(_) => {
                        vfslog!("[DevFS] Device opened successfully: {}", dev_name);
                        Ok(*inode_no)
                    }
                    Err(e) => {
                        vfslog!("[DevFS] Failed to open device: {}", dev_name);
                        Err(Error::from(e))
                    }
                }
            }
            None => {
                vfslog!("[DevFS] Device node not found: {}", dev_name);
                Err(code::ENOENT)
            }
        }
    }

    fn read(&self, inode_no: InodeNo, buf: &mut [u8], offset: &mut usize) -> Result<usize, Error> {
        self.check_mounted()?;

        let nodes = self.dev_nodes.read();
        let node = nodes.get(&inode_no).ok_or(code::ENOENT)?;
        let is_blocking = node.attr.mode & libc::O_NONBLOCK as u32 == 0;

        match node.dev.read(*offset, buf, is_blocking) {
            Ok(count) => {
                *offset += count;
                Ok(count)
            }
            Err(e) => Err(Error::from_errno(e as i32)),
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

        let nodes = self.dev_nodes.read();
        let node = nodes.get(&inode_no).ok_or(code::ENOENT)?;
        let is_blocking = node.attr.mode & libc::O_NONBLOCK as u32 == 0;

        match node.dev.write(*offset, buf, is_blocking) {
            Ok(count) => {
                *offset += count;
                Ok(count)
            }
            Err(e) => Err(Error::from_errno(e as i32)),
        }
    }

    fn close(&self, inode_no: InodeNo) -> Result<(), Error> {
        self.check_mounted()?;

        let nodes = self.dev_nodes.read();
        let node = nodes.get(&inode_no).ok_or(code::ENOENT)?;

        node.dev.close().map_err(|e| Error::from_errno(e as i32))
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
        let node = nodes.get_mut(&inode_no).ok_or(code::ENOENT)?;

        match whence {
            SEEK_SET => node.offset = offset,
            SEEK_CUR => node.offset += offset,
            SEEK_END => node.offset = 0, // For device files, we don't have a real end
            _ => return Err(code::EINVAL),
        }

        Ok(node.offset)
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
        _inode_no: InodeNo,
        offset: usize,
        dirents: &mut Vec<Dirent>,
        count: usize,
    ) -> Result<usize, Error> {
        self.check_mounted()?;

        let nodes = self.dev_nodes.read();

        // Collect and sort device nodes
        let mut entries: Vec<_> = nodes
            .iter()
            .map(|(_, node)| {
                let d_type = match node.attr.file_type {
                    FileType::Regular => DT_REG,
                    FileType::Directory => DT_DIR,
                    FileType::SymLink => DT_LNK,
                    FileType::CharDevice => DT_CHR,
                    FileType::BlockDevice => DT_BLK,
                };

                Dirent::new(d_type, node.dev.name().to_string())
            })
            .collect();

        // Sort by name
        entries.sort_by(|a, b| a.name.cmp(&b.name));

        // Check offset
        let start_idx = offset;
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
        if self.mounted.load(Ordering::Relaxed) {
            return Err(code::EEXIST);
        }

        *self.mount_point.write() = target.to_string();
        self.mounted.store(true, Ordering::Relaxed);
        self.scan_devices()?;

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
