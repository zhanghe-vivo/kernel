#![allow(dead_code)]
#[cfg(procfs)]
use crate::vfs::procfs::ProcFileSystem;
use crate::{
    error::{code, Error},
    vfs::{dcache::Dcache, fs::FileSystem, tmpfs::TmpFileSystem},
};

use alloc::{collections::BTreeMap, string::String, sync::Arc, vec::Vec};
use log::{debug, warn};
use spin::{Once, RwLock as SpinRwLock};

/// Mount point information
#[derive(Clone)]
pub struct MountPoint {
    /// Root directory entry
    pub root: Arc<Dcache>,
    /// Filesystem instance
    pub fs: Arc<dyn FileSystem>,
}

/// Mount point manager
#[cfg_attr(feature = "cbindgen", no_mangle)]
pub struct MountManager {
    /// List of mount points, sorted by path length in descending order to ensure longest match priority
    mount_points: SpinRwLock<BTreeMap<String, Arc<MountPoint>>>,
}

impl MountManager {
    pub fn new() -> Self {
        Self {
            mount_points: SpinRwLock::new(BTreeMap::new()),
        }
    }

    /// Add a mount point
    pub fn add_mount(
        &self,
        path: &String,
        root: Arc<Dcache>,
        fs: Arc<dyn FileSystem>,
    ) -> Result<(), Error> {
        let mut mounts = self.mount_points.write();
        if mounts.contains_key(path) {
            warn!("[mount_manager] Mount point already exists: {}", path);
            return Err(code::EEXIST);
        }

        mounts.insert(path.clone(), Arc::new(MountPoint { root, fs }));

        debug!("[mount_manager] Added mount point: {}", path);
        return Ok(());
    }

    pub fn remove_mount(&self, path: &String) -> Result<(), Error> {
        let mut mounts = self.mount_points.write();
        mounts.remove(path);
        Ok(())
    }

    pub fn find_mount(&self, path: &String) -> Option<Arc<MountPoint>> {
        self.mount_points.read().get(path).cloned()
    }

    /// Get all mount points
    #[allow(dead_code)]
    pub fn list_mounts(&self) -> Vec<Arc<MountPoint>> {
        self.mount_points.read().values().cloned().collect()
    }
}

/// Global mount manager instance
static MOUNT_MANAGER: Once<MountManager> = Once::new();
/// Get mount manager instance
#[inline(always)]
pub fn get_mount_manager() -> &'static MountManager {
    MOUNT_MANAGER.call_once(|| MountManager::new())
}

pub fn get_fs(fs_type: &str, _device: &str) -> Option<Arc<dyn FileSystem>> {
    match fs_type {
        "tmpfs" => Some(TmpFileSystem::new()),
        "devfs" => Some(TmpFileSystem::new()),
        #[cfg(procfs)]
        "procfs" => Some(ProcFileSystem::new()),
        // "fat32" => {
        //     let device = DeviceManager::get().get_block_device(device)?;
        //     Some(Arc::new(Fat32FileSystem::new())),
        // }
        _ => None,
    }
}
