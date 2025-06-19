use crate::{
    devices::{Device, DeviceManager},
    error::{code, Error},
    vfs::{inode_mode::InodeMode, path},
};
use alloc::sync::Arc;
use log::debug;

pub fn init() -> Result<(), Error> {
    DeviceManager::get().foreach(|name, dev| add_device(name, dev))
}

fn add_device(path: &str, dev: Arc<dyn Device>) -> Result<(), Error> {
    let mut dev_dir = path::lookup_path("/dev").ok_or(code::ENOENT)?;

    let mut rel_path = path.trim_start_matches('/');
    while !rel_path.is_empty() {
        match rel_path.split_once('/') {
            Some((next_name, next_path)) => {
                rel_path = next_path.trim_start_matches('/');
                dev_dir = dev_dir.lookup(next_name)?;
            }
            None => match dev_dir.lookup(rel_path) {
                Ok(_) => {
                    return Err(code::EEXIST);
                }
                Err(_) => {
                    dev_dir.create_device(
                        rel_path,
                        InodeMode::from_bits_truncate(0o666),
                        dev.clone(),
                    )?;
                    break;
                }
            },
        }
    }

    debug!("[devfs] Added device: {}", path);
    Ok(())
}
