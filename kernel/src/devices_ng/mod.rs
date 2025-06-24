use crate::error::Error;
use alloc::{collections::BTreeMap, string::String, sync::Arc};
use core::{
    fmt::Debug,
    sync::atomic::{AtomicU32, Ordering},
};
use embedded_io::ErrorKind;
use libc::*;
use spin::{Once, RwLock as SpinRwLock};

pub mod console;
pub(crate) mod dumb;
mod error;
mod null;
#[cfg(target_arch = "arm")]
pub(crate) mod nvic;
#[cfg(target_arch = "riscv64")]
pub(crate) mod plic;
pub mod serial;
#[cfg(virtio)]
pub mod virtio;
mod zero;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DeviceClass {
    Char,
    Block,
    Misc,
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
    NotSupported = 0x00, // not supported
}

impl From<u32> for DeviceRequest {
    fn from(value: u32) -> Self {
        match value {
            0x01 => Self::Resume,
            0x02 => Self::Suspend,
            0x03 => Self::Config,
            0x04 => Self::Close,
            _ => Self::NotSupported,
        }
    }
}

/// Mask for control commands
pub const DEVICE_GENERAL_REQUEST_MASK: u32 = 0x1f;

#[derive(Debug)]
pub struct DeviceBase {
    pub open_count: AtomicU32,
}

impl DeviceBase {
    pub fn new() -> Self {
        Self {
            open_count: AtomicU32::new(0),
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
}

#[allow(unused)]
#[derive(Debug, Clone, Copy)]
pub struct DeviceId {
    major: u32,
    minor: u32,
}

#[allow(unused_variables)]
pub trait Device: Send + Sync {
    fn name(&self) -> String;
    fn class(&self) -> DeviceClass;
    fn id(&self) -> DeviceId;
    fn open(&self) -> Result<(), ErrorKind> {
        Ok(())
    }
    fn close(&self) -> Result<(), ErrorKind> {
        Ok(())
    }
    fn read(&self, pos: usize, buf: &mut [u8], is_nonblocking: bool) -> Result<usize, ErrorKind>;
    fn write(&self, pos: usize, buf: &[u8], is_nonblocking: bool) -> Result<usize, ErrorKind>;
    fn ioctl(&self, request: u32, arg: usize) -> Result<(), ErrorKind> {
        Err(ErrorKind::Unsupported)
    }
}

impl Debug for dyn Device {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("Device")
            .field("name", &self.name())
            .field("class", &self.class())
            .field("id", &self.id())
            .finish()
    }
}

static DEVICE_MANAGER: Once<DeviceManager> = Once::new();

pub struct DeviceManager {
    pub char_devices: SpinRwLock<BTreeMap<String, Arc<dyn Device>>>,
    pub block_devices: SpinRwLock<BTreeMap<String, Arc<dyn Device>>>,
    pub misc_devices: SpinRwLock<BTreeMap<String, Arc<dyn Device>>>,
}

impl DeviceManager {
    pub fn new() -> Self {
        Self {
            char_devices: SpinRwLock::new(BTreeMap::new()),
            block_devices: SpinRwLock::new(BTreeMap::new()),
            misc_devices: SpinRwLock::new(BTreeMap::new()),
        }
    }

    pub fn get() -> &'static DeviceManager {
        DEVICE_MANAGER.call_once(|| DeviceManager::new())
    }

    pub fn get_device_number(&self) -> usize {
        self.char_devices.read().len()
            + self.block_devices.read().len()
            + self.misc_devices.read().len()
    }

    pub fn register_device(&self, name: String, dev: Arc<dyn Device>) -> Result<(), ErrorKind> {
        match dev.class() {
            DeviceClass::Char => {
                let mut devices = self.char_devices.write();
                devices
                    .try_insert(name, dev)
                    .map_err(|_| ErrorKind::AlreadyExists)?;
            }
            DeviceClass::Block => {
                let mut devices = self.block_devices.write();
                devices
                    .try_insert(name, dev)
                    .map_err(|_| ErrorKind::AlreadyExists)?;
            }
            DeviceClass::Misc => {
                let mut devices = self.misc_devices.write();
                devices
                    .try_insert(name, dev)
                    .map_err(|_| ErrorKind::AlreadyExists)?;
            }
        };
        Ok(())
    }

    pub fn get_block_device(&self, str: &str) -> Option<Arc<dyn Device>> {
        self.block_devices.read().get(str).cloned()
    }

    pub fn get_char_device(&self, str: &str) -> Option<Arc<dyn Device>> {
        self.char_devices.read().get(str).cloned()
    }

    pub fn get_misc_device(&self, str: &str) -> Option<Arc<dyn Device>> {
        self.misc_devices.read().get(str).cloned()
    }

    pub fn foreach<F>(&self, callback: F) -> Result<(), Error>
    where
        F: Fn(&str, Arc<dyn Device>) -> Result<(), Error>,
    {
        {
            let char_devices = self.char_devices.read();
            for (name, device) in char_devices.iter() {
                callback(name, device.clone())?
            }
        }
        {
            let block_devices = self.block_devices.read();
            for (name, device) in block_devices.iter() {
                callback(name, device.clone())?
            }
        }
        {
            let misc_devices = self.misc_devices.read();
            for (name, device) in misc_devices.iter() {
                callback(name, device.clone())?
            }
        }
        Ok(())
    }
}

pub fn init() -> Result<(), Error> {
    null::Null::register().map_err(|e| Error::from(e))?;
    zero::Zero::register().map_err(|e| Error::from(e))?;
    Ok(())
}
