use crate::vfs::vfs_mode::AccessMode;
use alloc::{collections::BTreeMap, sync::Arc};
use core::sync::atomic::{AtomicU32, Ordering};
use embedded_io::ErrorKind;
use libc::*;
use safe_mmio::UniqueMmioPointer;
use spin::{Once, RwLock as SpinRwLock};

pub mod console;
mod error;
#[cfg(fdt)]
pub mod fdt;
mod null;
pub mod serial;
#[cfg(virtio)]
pub mod virtio;
mod zero;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DeviceClass {
    Char,
    Block,
    Net,
}

/// general device commands
///
/// - 0x01 - 0x1F: general device control commands
/// - 0x20 - 0x3F: udevice control commands
/// - 0x40+: special device control commands
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceRequest {
    Resume = 0x01,       // resume device
    Suspend = 0x02,      // suspend device
    Config = 0x03,       // configure device
    Close = 0x04,        // close device
    NotifySet = 0x05,    // set notify func
    SetInt = 0x06,       // set interrupt
    ClrInt = 0x07,       // clear interrupt
    GetInt = 0x08,       // get interrupt status
    ConsoleOflag = 0x09, // get console open flag
    NotSupported = 0x00, // not supported
}

impl From<u32> for DeviceRequest {
    fn from(value: u32) -> Self {
        match value {
            0x01 => Self::Resume,
            0x02 => Self::Suspend,
            0x03 => Self::Config,
            0x04 => Self::Close,
            0x05 => Self::NotifySet,
            0x06 => Self::SetInt,
            0x07 => Self::ClrInt,
            0x08 => Self::GetInt,
            0x09 => Self::ConsoleOflag,
            _ => Self::NotSupported,
        }
    }
}

/// Mask for control commands
pub const DEVICE_GENERAL_REQUEST_MASK: u32 = 0x1f;

#[derive(Debug)]
pub struct DeviceBase {
    pub name: &'static str,
    pub open_count: AtomicU32,
    pub class: DeviceClass,
    pub access_mode: AccessMode,
}

impl DeviceBase {
    pub fn new(name: &'static str, device_class: DeviceClass, access_mode: AccessMode) -> Self {
        Self {
            name,
            open_count: AtomicU32::new(0),
            class: device_class,
            access_mode,
        }
    }

    pub fn check_permission(&self, oflag: i32) -> Result<(), ErrorKind> {
        let access_mode = AccessMode::from(oflag);

        // Check if requested access mode is compatible with device's access mode
        match (access_mode, self.access_mode) {
            (AccessMode::O_RDONLY, AccessMode::O_WRONLY)
            | (AccessMode::O_WRONLY, AccessMode::O_RDONLY)
            | (AccessMode::O_RDWR, AccessMode::O_WRONLY)
            | (AccessMode::O_RDWR, AccessMode::O_RDONLY) => Err(ErrorKind::PermissionDenied),
            _ => Ok(()),
        }
    }

    pub fn inc_open_count(&self) -> u32 {
        self.open_count.fetch_add(1, Ordering::Relaxed)
    }

    pub fn dec_open_count(&self) -> u32 {
        self.open_count.fetch_sub(1, Ordering::Relaxed)
    }

    pub fn is_opened(&self) -> bool {
        self.open_count.load(Ordering::Relaxed) > 0
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn class(&self) -> DeviceClass {
        self.class
    }

    pub fn access_mode(&self) -> AccessMode {
        self.access_mode
    }

    pub fn set_name(&mut self, name: &'static str) {
        self.name = name;
    }

    pub fn set_access_mode(&mut self, access_mode: AccessMode) {
        self.access_mode = access_mode;
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DeviceId {
    major: u32,
    minor: u32,
}

pub trait Device: Send + Sync {
    fn name(&self) -> &'static str;
    fn class(&self) -> DeviceClass;
    fn access_mode(&self) -> AccessMode;
    fn id(&self) -> DeviceId;
    fn read(&self, pos: usize, buf: &mut [u8], is_blocking: bool) -> Result<usize, ErrorKind>;
    fn write(&self, pos: usize, buf: &[u8], is_blocking: bool) -> Result<usize, ErrorKind>;
    fn open(&self, oflag: i32) -> Result<(), ErrorKind>;
    fn close(&self) -> Result<(), ErrorKind>;
    fn ioctl(&self, _request: u32, _arg: usize) -> Result<(), ErrorKind> {
        Err(ErrorKind::Unsupported)
    }
}

static DEVICE_MANAGER: Once<DeviceManager> = Once::new();

pub struct DeviceManager {
    pub devices: SpinRwLock<BTreeMap<&'static str, Arc<dyn Device>>>,
}

impl DeviceManager {
    pub fn new() -> Self {
        Self {
            devices: SpinRwLock::new(BTreeMap::new()),
        }
    }

    pub fn get() -> &'static DeviceManager {
        DEVICE_MANAGER.call_once(|| DeviceManager::new())
    }

    pub fn get_device_number(&self) -> usize {
        self.devices.read().len()
    }

    pub fn find_device(&self, name: &str) -> Result<Arc<dyn Device>, ErrorKind> {
        self.devices
            .read()
            .get(name)
            .map_or(Err(ErrorKind::NotFound), |device| Ok(device.clone()))
    }

    pub fn register_device(
        &self,
        name: &'static str,
        dev: Arc<dyn Device>,
    ) -> Result<(), ErrorKind> {
        let mut devices = self.devices.write();
        if devices.contains_key(name) {
            return Err(ErrorKind::AlreadyExists);
        }
        devices.insert(name, dev);
        Ok(())
    }

    pub fn unregister_device(&self, name: &str) -> Result<(), ErrorKind> {
        let mut devices = self.devices.write();
        devices.remove(name);
        Ok(())
    }

    pub fn foreach<F>(&self, callback: F) -> Result<(), ErrorKind>
    where
        F: Fn(&'static str, Arc<dyn Device>) -> Result<(), ErrorKind>,
    {
        for (name, device) in self.devices.read().iter() {
            callback(name, device.clone())?
        }
        Ok(())
    }
}

pub fn init() -> Result<(), ErrorKind> {
    null::Null::register()?;
    zero::Zero::register()?;
    Ok(())
}
