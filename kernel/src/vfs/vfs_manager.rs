//! VFS (Virtual File System) Manager implementation
#![allow(dead_code)]

use crate::{
    error::{code, Error},
    vfs::{vfs_log::*, vfs_traits::VfsOperations},
};
use alloc::{collections::BTreeMap, string::String, sync::Arc};
use spin::{Lazy, RwLock as SpinRwLock};

/// File System Manager
#[cfg_attr(feature = "cbindgen", no_mangle)]
pub struct VfsManager {
    /// Registered filesystem types
    fs_types: SpinRwLock<BTreeMap<String, Arc<dyn VfsOperations>>>,
}

impl VfsManager {
    /// Create a new VFS manager
    pub fn new() -> Self {
        VfsManager {
            fs_types: SpinRwLock::new(BTreeMap::new()),
        }
    }

    /// Register a filesystem type
    pub fn register_fs(&self, name: &str, fs: Arc<dyn VfsOperations>) -> Result<(), Error> {
        let mut fs_types = self.fs_types.write();

        if fs_types.contains_key(name) {
            vfslog!("Filesystem {} already exists", name);
            return Err(code::EEXIST);
        }

        fs_types.insert(String::from(name), fs);
        vfslog!("Registered filesystem: {}", name);
        Ok(())
    }

    /// Unregister a filesystem type
    pub fn unregister_fs(&self, name: &str) -> Result<(), Error> {
        let mut fs_types = self.fs_types.write();

        if fs_types.remove(name).is_none() {
            vfslog!("Filesystem {} not found", name);
            return Err(code::ENOENT);
        }

        vfslog!("Unregistered filesystem: {}", name);
        Ok(())
    }

    /// Get a registered filesystem type
    pub fn get_fs(&self, name: &str) -> Option<Arc<dyn VfsOperations>> {
        self.fs_types.read().get(name).cloned()
    }
}

impl Default for VfsManager {
    fn default() -> Self {
        Self::new()
    }
}

// Global VFS manager instance
static VFS_MANAGER: Lazy<VfsManager> = Lazy::new(|| VfsManager::new());

// Get VFS manager instance
#[inline(always)]
pub fn get_vfs_manager() -> &'static VfsManager {
    &VFS_MANAGER
}
