use crate::vfs::{dcache::Dcache, fs::FileSystem, mount::get_mount_manager, tmpfs::TmpFileSystem};
use alloc::{string::String, sync::Arc};
use spin::Once;

static ROOT_DIR: Once<Arc<Dcache>> = Once::new();
pub fn init() {
    ROOT_DIR.call_once(|| -> Arc<Dcache> {
        let rootfs = TmpFileSystem::new();
        let root = Dcache::new_root(rootfs.root_inode());
        get_mount_manager()
            .add_mount(&String::from("/"), root.clone(), rootfs.clone())
            .unwrap();
        root
    });
}

pub fn get_root_dir() -> &'static Arc<Dcache> {
    ROOT_DIR.get().unwrap()
}
