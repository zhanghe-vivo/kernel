#[cfg(procfs)]
use crate::vfs::procfs::ProcFileSystem;
use crate::{
    error::Error,
    vfs::{
        dcache::Dcache,
        fd_manager::get_fd_manager,
        fs::FileSystem,
        inode_mode::{InodeFileType, InodeMode},
        mount::get_mount_manager,
        tmpfs::TmpFileSystem,
    },
};
use log::{debug, error, warn};

mod dcache;
mod devfs;
pub mod dirent;
#[cfg(virtio)]
mod fatfs;
mod fd_manager;
mod file;
mod fs;
mod inode;
mod inode_mode;
mod mount;
mod path;
mod root;
pub mod syscalls;
mod tmpfs;
mod utils;
use alloc::string::String;
pub use file::AccessMode;
use semihosting::println;

/// Initialize the virtual file system  
pub fn vfs_init() -> Result<(), Error> {
    debug!("Initializing VFS...");
    root::init();
    let cwd = path::get_working_dir();

    // /dev is a temporary filesystem
    let dev_name = String::from("dev");
    let devfs = TmpFileSystem::new();
    // create the directory /dev
    let dev_dir = cwd.new_child(
        dev_name.as_str(),
        InodeFileType::Directory,
        InodeMode::from(0o555),
        || None,
    )?;
    let devfs_mount_point = Dcache::new(devfs.root_inode(), dev_name, cwd.get_weak_ref());
    devfs_mount_point.mount(devfs)?;
    debug!("Mounted devfs at '/dev'");
    devfs::init()?;

    debug!("init stdio");
    let mut fd_manager = get_fd_manager().lock();
    fd_manager.init_stdio()?;

    #[cfg(virtio)]
    {
        use crate::{
            devices::block::VIRTUAL_STORAGE_NAME,
            vfs::{fatfs::FatFileSystem, fs::FileSystem},
        };
        use alloc::string::String;

        debug!("init fatfs");
        match FatFileSystem::new(VIRTUAL_STORAGE_NAME) {
            Ok(fatfs) => {
                let fat_name = String::from("fat");
                // create the directory /fat
                let dev_dir = cwd.new_child(
                    fat_name.as_str(),
                    InodeFileType::Directory,
                    InodeMode::from(0o555),
                    || None,
                )?;
                let fatfs_mount_point =
                    Dcache::new(fatfs.root_inode(), fat_name, cwd.get_weak_ref());
                fatfs_mount_point.mount(fatfs)?;
                debug!("Mounted fatfs at '/fat'");
            }
            Err(error) => {
                error!("Fail to init fat file system, {}", error);
                return Err(error);
            }
        }
    }

    #[cfg(procfs)]
    {
        use alloc::sync::Arc;
        // Register procfs filesystem
        let procfs = Arc::new(ProcFileSystem::new());
        vfs_manager.register_fs("procfs", procfs.clone())?;
        // Mount procfs to /proc
        if vfs_posix::mount(None, "/proc", "procfs", 0, None) == 0 {
            debug!("Mounted procfs at '/proc'");
        } else {
            warn!("Failed to mount procfs");
            return Err(code::EAGAIN);
        }

        ProcFileSystem::init()?;
    }

    debug!("VFS initialized successfully");
    Ok(())
}
