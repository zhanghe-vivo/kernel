#[cfg(procfs)]
use crate::vfs::procfs::ProcFileSystem;
use crate::{
    error::Error,
    vfs::{
        fd_manager::get_fd_manager,
        inode_mode::{InodeFileType, InodeMode},
        tmpfs::TmpFileSystem,
    },
};
use log::{debug, warn};

mod dcache;
mod devfs;
pub mod dirent;
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
pub use file::AccessMode;

/// Initialize the virtual file system  
pub fn vfs_init() -> Result<(), Error> {
    debug!("Initializing VFS...");
    root::init();
    let cwd = path::get_working_dir();
    let dev_dir = cwd.new_child("dev", InodeFileType::Directory, InodeMode::from(0o555))?;
    // /dev is a temporary filesystem
    let devfs = TmpFileSystem::new();
    match dev_dir.mount(devfs) {
        Ok(_) => debug!("Mounted devfs at '/dev'"),
        Err(e) => {
            warn!("Failed to mount devfs: {}", e);
            return Err(e);
        }
    }
    devfs::init()?;

    debug!("init stdio");
    let mut fd_manager = get_fd_manager().lock();
    fd_manager.init_stdio()?;

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
