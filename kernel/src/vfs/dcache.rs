#![allow(dead_code)]
use crate::{
    devices::Device,
    error::{code, Error},
    vfs::{
        fs::{FileSystem, FileSystemInfo},
        inode::InodeOps,
        inode_mode::{InodeFileType, InodeMode},
        utils::NAME_MAX,
    },
};
use alloc::{
    collections::BTreeMap,
    format,
    string::String,
    sync::{Arc, Weak},
};
use core::{
    fmt::{self, Debug},
    ptr,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use delegate::delegate;
use log::{debug, warn};
use spin::RwLock;

/// File system lookup cache
pub struct Dcache {
    // inode will never change after creation
    inode: Arc<dyn InodeOps>,
    // name and parent may change by rename, None means root directory
    name_and_parent: RwLock<Option<(String, Arc<Dcache>)>>,
    children: RwLock<BTreeMap<String, Arc<Dcache>>>,
    // use to set parent in children
    this: Weak<Dcache>,
    is_mount_point: AtomicBool,
}

impl Dcache {
    /// Create new directory node
    pub fn new(inode: Arc<dyn InodeOps>, name: String, parent: Arc<Dcache>) -> Arc<Self> {
        Arc::new_cyclic(|weak_self| Self {
            inode,
            name_and_parent: RwLock::new(Some((name, parent))),
            children: RwLock::new(BTreeMap::new()),
            this: weak_self.clone(),
            is_mount_point: AtomicBool::new(false),
        })
    }

    pub fn new_root(inode: Arc<dyn InodeOps>) -> Arc<Self> {
        Arc::new_cyclic(|weak_self| Self {
            inode,
            name_and_parent: RwLock::new(None),
            children: RwLock::new(BTreeMap::new()),
            this: weak_self.clone(),
            is_mount_point: AtomicBool::new(false),
        })
    }

    pub fn new_child(
        &self,
        name: &str,
        type_: InodeFileType,
        mode: InodeMode,
    ) -> Result<Arc<Self>, Error> {
        if self.inode.type_() != InodeFileType::Directory {
            return Err(code::ENOTDIR);
        }
        if name == "." || name == ".." {
            return Err(code::EEXIST);
        }
        let mut children = self.children.write();
        if children.contains_key(name) {
            return Err(code::EEXIST);
        }

        let inode = self.inode.create(name, type_, mode)?;
        let child = Self::new(inode, String::from(name), self.this.upgrade().unwrap());
        children.insert(String::from(name), child.clone());
        Ok(child)
    }

    pub fn create_device(
        &self,
        name: &str,
        mode: InodeMode,
        dev: Arc<dyn Device>,
    ) -> Result<Arc<Self>, Error> {
        if self.inode.type_() != InodeFileType::Directory {
            return Err(code::ENOTDIR);
        }
        let mut children = self.children.write();
        if children.contains_key(name) {
            return Err(code::EEXIST);
        }

        let inode = self.inode.create_device(name, mode, dev)?;
        let name_str = String::from(name);
        let child = Self::new(inode, name_str.clone(), self.this.upgrade().unwrap());
        children.insert(name_str, child.clone());
        Ok(child)
    }

    pub fn lookup(&self, name: &str) -> Result<Arc<Dcache>, Error> {
        if name.len() > NAME_MAX {
            return Err(code::ENAMETOOLONG);
        }

        if self.inode.type_() != InodeFileType::Directory {
            return Err(code::ENOTDIR);
        }

        if !self.inode.mode().contains(InodeMode::S_IXUSR) {
            return Err(code::ENOTDIR);
        }

        let this = self.this.upgrade().unwrap();
        let entry = match name {
            "." => this,
            ".." => self.parent().unwrap_or(this),
            name => {
                // find in dcache first
                match self.find_child(name) {
                    Some(child) => child,
                    None => {
                        // lookup in filesystem
                        let inode = self.inode.lookup(name)?;
                        let entry = Dcache::new(inode, String::from(name), this.clone());
                        self.add_child(String::from(name), &entry);
                        entry
                    }
                }
            }
        };

        Ok(entry)
    }

    /// Add child node
    pub fn add_child(&self, name: String, child: &Arc<Dcache>) {
        self.children.write().insert(name, child.clone());
    }

    /// Remove child node
    pub fn remove_child(&self, name: &str) -> Option<Arc<Dcache>> {
        self.children.write().remove(name)
    }

    /// Find child node
    pub fn find_child(&self, name: &str) -> Option<Arc<Dcache>> {
        let children = self.children.read();
        match children.get(name) {
            Some(child) => Some(child.clone()),
            None => None,
        }
    }

    fn set_name_and_parent(&self, name: &str, parent: Arc<Self>) {
        let mut name_and_parent = self.name_and_parent.write();
        *name_and_parent = Some((String::from(name), parent));
    }

    fn name(&self) -> String {
        match self.name_and_parent.read().as_ref() {
            Some(x) => x.0.clone(),
            None => String::from("/"),
        }
    }

    fn parent(&self) -> Option<Arc<Dcache>> {
        self.name_and_parent.read().as_ref().map(|x| x.1.clone())
    }

    pub fn inode(&self) -> &Arc<dyn InodeOps> {
        &self.inode
    }

    pub fn fs_info(&self) -> FileSystemInfo {
        self.inode.fs().fs_info()
    }

    pub fn is_mount_point(&self) -> bool {
        self.is_mount_point.load(Ordering::Relaxed)
    }

    /// Get full path
    pub fn get_full_path(&self) -> String {
        // Handle root directory case
        if self.name_and_parent.read().as_ref().is_none() {
            return String::from("/");
        }

        let mut path = self.name();
        let mut current = self.parent();

        while let Some(node) = current {
            let parent_name = node.name();
            if parent_name != "/" {
                path = format!("{}/{}", parent_name, path);
            }
            current = node.parent();
        }
        path = format!("/{}", path);
        path
    }

    pub fn mount(&self, fs: Arc<dyn FileSystem>) -> Result<(), Error> {
        if self.inode.type_() != InodeFileType::Directory {
            return Err(code::ENOTDIR);
        }

        if self.is_mount_point() {
            warn!("Directory is already a mount point");
            return Err(code::EBUSY);
        }

        fs.mount(self.this.upgrade().unwrap())?;
        self.is_mount_point.store(true, Ordering::Relaxed);
        Ok(())
    }

    pub fn unmount(&self) -> Result<(), Error> {
        if !self.is_mount_point() {
            warn!("Directory is not a mount point");
            return Err(code::EINVAL);
        }

        self.inode.fs().unmount()?;
        self.is_mount_point.store(false, Ordering::Relaxed);
        Ok(())
    }

    pub fn link(&self, old: &Arc<Dcache>, new_name: &str) -> Result<(), Error> {
        if self.inode.type_() != InodeFileType::Directory {
            return Err(code::ENOTDIR);
        }
        let mut children = self.children.write();
        if children.contains_key(new_name) {
            return Err(code::EEXIST);
        }

        self.inode.link(&old.inode, new_name)?;
        let new_name_str = String::from(new_name);
        let new_child = Self::new(
            old.inode.clone(),
            new_name_str.clone(),
            self.this.upgrade().unwrap(),
        );
        children.insert(new_name_str, new_child);
        Ok(())
    }

    /// Unlink a file or directory
    pub fn unlink(&self, name: &str) -> Result<(), Error> {
        if self.inode.type_() != InodeFileType::Directory {
            return Err(code::ENOTDIR);
        }
        let mut children = self.children.write();
        let Some(child) = children.get(name) else {
            return Err(code::ENOENT);
        };
        if child.is_mount_point() {
            return Err(code::EBUSY);
        }

        self.inode.unlink(name)?;
        children.remove(name);
        Ok(())
    }

    pub fn rmdir(&self, name: &str) -> Result<(), Error> {
        if self.inode.type_() != InodeFileType::Directory {
            return Err(code::ENOTDIR);
        }
        let mut children = self.children.write();
        let Some(child) = children.get(name) else {
            return Err(code::ENOENT);
        };
        if child.is_mount_point() {
            return Err(code::EBUSY);
        }
        self.inode.rmdir(name)?;
        children.remove(name);
        Ok(())
    }

    pub fn rename(
        &self,
        old_name: &str,
        new_dir: &Arc<Dcache>,
        new_name: &str,
    ) -> Result<(), Error> {
        if self.inode.type_() != InodeFileType::Directory
            || new_dir.inode.type_() != InodeFileType::Directory
        {
            return Err(code::ENOTDIR);
        }

        if old_name == "." || old_name == ".." || new_name == "." || new_name == ".." {
            warn!("Invalid name: {} to {}", old_name, new_name);
            return Err(code::EINVAL);
        }

        let mut children = self.children.write();
        let child = match children.get(old_name) {
            Some(child) => child.clone(),
            None => {
                debug!("{} not found", old_name);
                return Err(code::ENOENT);
            }
        };
        if child.is_mount_point() {
            debug!("{} is a mount point", old_name);
            return Err(code::EBUSY);
        }

        // rename in the same directory
        if ptr::addr_eq(self, Arc::as_ptr(&new_dir)) && old_name != new_name {
            if children.contains_key(new_name) {
                debug!("{} already exists", new_name);
                return Err(code::EEXIST);
            }
            self.inode.rename(old_name, &self.inode, new_name)?;
            children.remove(old_name);
            children.insert(String::from(new_name), child);
        } else {
            let mut new_children = new_dir.children.write();
            if new_children.contains_key(new_name) {
                debug!("{} already exists", new_name);
                return Err(code::EEXIST);
            }
            self.inode.rename(old_name, &new_dir.inode, new_name)?;
            children.remove(old_name);
            child.set_name_and_parent(new_name, new_dir.clone());
            new_children.insert(String::from(new_name), child);
        }

        Ok(())
    }

    delegate! {
        to self.inode {
            pub fn fs(&self) -> Arc<dyn FileSystem>;
            pub fn fsync(&self) -> Result<(), Error>;
            pub fn size(&self) -> usize;
            pub fn resize(&self, size: usize) -> Result<(), Error>;
            pub fn type_(&self) -> InodeFileType;
            pub fn mode(&self) -> InodeMode;
            pub fn atime(&self) -> Duration;
            pub fn set_atime(&self, time: Duration);
            pub fn mtime(&self) -> Duration;
            pub fn set_mtime(&self, time: Duration);
        }
    }
}

impl Debug for Dcache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Dcache {{ inode: {:p}, name: {} }}",
            self.inode,
            self.name()
        )
    }
}
