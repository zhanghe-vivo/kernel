#![allow(dead_code)]

use crate::{
    error::{code, Error},
    vfs::{vfs_path, vfs_traits::VfsOperations},
};
use alloc::{
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use log::{info, warn};
use spin::{Lazy, RwLock as SpinRwLock};

const RESERVED_MOUNT_PATH: &[&str] = &[
    #[cfg(procfs)]
    "/proc",
    "/dev",
];
/// Mount point information
#[derive(Clone)]
pub struct MountPoint {
    /// Mount point path
    pub path: String,
    /// Filesystem type
    pub fs_type: String,
    /// Device name (optional)
    pub device: Option<String>,
    /// Mount flags
    pub flags: u64,
    /// Filesystem instance
    pub fs: Arc<dyn VfsOperations>,
}

/// Mount point manager
#[cfg_attr(feature = "cbindgen", no_mangle)]
pub struct MountManager {
    /// List of mount points, sorted by path length in descending order to ensure longest match priority
    mount_points: SpinRwLock<Vec<MountPoint>>,
}

impl MountManager {
    pub fn new() -> Self {
        Self {
            mount_points: SpinRwLock::new(Vec::new()),
        }
    }

    /// Check if path is already mounted
    #[allow(dead_code)]
    pub fn is_mounted(&self, path: &str) -> bool {
        let normalized_path = match vfs_path::normalize_path(path) {
            Some(p) => p,
            None => return false,
        };

        let mounts = self.mount_points.read();
        mounts.iter().any(|m| m.path == normalized_path)
    }

    /// Add a mount point
    pub fn add_mount(&self, mount: MountPoint) -> Result<(), Error> {
        let mut mounts = self.mount_points.write();

        info!(
            "[mount_manager] Adding mount point: {} (type: {})",
            mount.path, mount.fs_type
        );

        // Normalize mount point path
        let normalized_path = match vfs_path::normalize_path(&mount.path) {
            Some(p) => p,
            None => {
                warn!("[mount_manager] Invalid mount path: {}", mount.path);
                return Err(code::EINVAL);
            }
        };

        // Check if mount point already exists
        if mounts.iter().any(|m| m.path == normalized_path) {
            warn!(
                "[mount_manager] Mount point already exists: {}",
                normalized_path
            );
            return Err(code::EEXIST);
        }

        // Special handling for /dev directory mount
        if normalized_path == "/dev" {
            info!("[mount_manager] Special handling for /dev mount");
            let new_mount = MountPoint {
                path: normalized_path.clone(),
                ..mount
            };
            mounts.push(new_mount);
            mounts.sort_by(|a, b| b.path.len().cmp(&a.path.len()));
            info!("[mount_manager] Successfully added /dev mount point");
            return Ok(());
        }

        // Keep the handling of other mount points unchanged...
        mounts.push(mount);
        mounts.sort_by(|a, b| b.path.len().cmp(&a.path.len()));

        info!("[mount_manager] Added mount point: {}", normalized_path);
        return Ok(());
    }

    pub fn remove_mount(&self, path: &str) -> Result<(), Error> {
        let mut mounts = self.mount_points.write();

        if let Some(index) = mounts.iter().position(|m| m.path == path) {
            mounts.remove(index);
            info!("Removed mount point: {}", path);
            Ok(())
        } else {
            warn!("Mount point not found: {}", path);
            Err(code::ENOENT)
        }
    }

    /// Find mount point for the given path
    pub fn find_mount(&self, path: &str) -> Option<MountPoint> {
        let mounts = self.mount_points.read();

        // Normalize search path
        let normalized_path = vfs_path::normalize_path(path)?;

        // Exact match takes priority
        if let Some(mount) = mounts.iter().find(|m| m.path == normalized_path) {
            return Some(mount.clone());
        }

        // Special handling for deserved path
        if RESERVED_MOUNT_PATH
            .iter()
            .any(|&reserved_path| path == reserved_path)
        {
            // If it's a mount operation, return None to allow mounting
            return None;
        }

        // Find longest prefix match
        let mut longest_match: Option<&MountPoint> = None;
        let mut longest_length = 0;
        let hited_deserved_path = RESERVED_MOUNT_PATH.iter().find(|&&deserved_path| {
            normalized_path.starts_with(&(deserved_path.to_string() + "/"))
        });
        for mount in mounts.iter() {
            if let Some(&deserved_path) = hited_deserved_path {
                if mount.path == deserved_path {
                    return Some(mount.clone());
                }
                if mount.path == "/" {
                    continue;
                }
            }

            if normalized_path.starts_with(&mount.path) {
                let is_valid_match = mount.path == "/"
                    || normalized_path.len() == mount.path.len()
                    || normalized_path.chars().nth(mount.path.len()) == Some('/');

                if is_valid_match && mount.path.len() > longest_length {
                    longest_match = Some(mount);
                    longest_length = mount.path.len();
                }
            }
        }

        if let Some(mount) = longest_match {
            Some(mount.clone())
        } else {
            None
        }
    }

    /// Get all mount points
    #[allow(dead_code)]
    pub fn list_mounts(&self) -> Vec<MountPoint> {
        self.mount_points.read().clone()
    }
}

/// Global mount manager instance
static MOUNT_MANAGER: Lazy<MountManager> = Lazy::new(|| MountManager::new());

/// Get mount manager instance
#[inline(always)]
pub fn get_mount_manager() -> &'static MountManager {
    &MOUNT_MANAGER
}

/// Find filesystem based on path
pub fn find_filesystem(path: &str) -> Option<(Arc<dyn VfsOperations>, String)> {
    let mount_manager = get_mount_manager();

    // Normalize path
    let normalized_path = vfs_path::normalize_path(path)?;

    // Find the longest matching mount point
    mount_manager
        .find_mount(&normalized_path)
        .map(|mp: MountPoint| {
            // Calculate relative path
            let relative_path = if mp.path == "/" {
                // Special handling for root mount point
                if normalized_path == "/" {
                    String::from("/")
                } else {
                    normalized_path
                }
            } else {
                // Remove mount point path prefix
                let rel_path = &normalized_path[mp.path.len()..];
                if rel_path.is_empty() {
                    String::from("/")
                } else {
                    rel_path.to_string()
                }
            };

            (mp.fs.clone(), relative_path)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bluekernel_test_macro::test;

    #[test]
    fn test_mount() {
        let mount_manager = get_mount_manager();

        // Test normalize path and exact match
        let test_path = "/dev/1/..";
        let expected_mount_path = "/dev";
        assert_eq!(
            mount_manager.find_mount(test_path).unwrap().path,
            expected_mount_path
        );

        // Test longest prefix match, the path start with deserved values
        let test_path = "/dev/1/2";
        let expected_mount_path = "/dev";
        assert_eq!(
            mount_manager.find_mount(test_path).unwrap().path,
            expected_mount_path
        );

        // Test longest prefix match, the path does not start with deserved values
        let test_path = "/dev2/1/2";
        let expected_mount_path = "/";
        assert_eq!(
            mount_manager.find_mount(test_path).unwrap().path,
            expected_mount_path
        );
    }
}
