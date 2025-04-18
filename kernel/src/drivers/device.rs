use alloc::{collections::BTreeMap, sync::Arc};
use bitflags::bitflags;
use core::sync::atomic::{AtomicI32, AtomicU32, Ordering};
use embedded_io::ErrorKind;
use libc::*;
use spin::{Once, RwLock as SpinRwLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceClass {
    Char,
    Block,
    // NetIf,
    // MTD,
    // CAN,
    // RTC,
    // Sound,
    // Graphic,
    // ...
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

// Device Flags
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DeviceFlags: u32 {
        const RDONLY = O_RDONLY as u32;
        const WRONLY = O_WRONLY as u32;
        const RDWR = O_RDWR as u32;
        const STREAM = 0x1000;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceId {
    pub major: u32,
    pub minor: u32,
}

#[derive(Debug)]
pub struct DeviceBase {
    pub name: &'static str,
    pub class: DeviceClass,
    pub flags: DeviceFlags,
    pub oflag: AtomicI32,
    pub open_count: AtomicU32,
}

impl DeviceBase {
    pub fn new(name: &'static str, device_class: DeviceClass, device_flags: DeviceFlags) -> Self {
        Self {
            name,
            class: device_class,
            flags: device_flags,
            oflag: AtomicI32::new(0),
            open_count: AtomicU32::new(0),
        }
    }

    pub fn check_flags(&self, oflag: i32) -> Result<(), ErrorKind> {
        let flags = DeviceFlags::from_bits_truncate(oflag as u32);

        // Check if requested flags are compatible with device flags
        if flags.contains(DeviceFlags::RDONLY) && !self.flags.contains(DeviceFlags::RDONLY) {
            return Err(ErrorKind::PermissionDenied);
        }
        if flags.contains(DeviceFlags::WRONLY) && !self.flags.contains(DeviceFlags::WRONLY) {
            return Err(ErrorKind::PermissionDenied);
        }

        Ok(())
    }

    pub fn set_oflag(&self, oflag: i32) {
        self.oflag.store(oflag, Ordering::Relaxed);
    }

    pub fn oflag(&self) -> i32 {
        self.oflag.load(Ordering::Relaxed)
    }

    pub fn is_blocking(&self) -> bool {
        (self.oflag() & O_NONBLOCK) == 0
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

    pub fn flags(&self) -> DeviceFlags {
        self.flags
    }
}

pub trait Device: Send + Sync {
    fn name(&self) -> &'static str;
    fn class(&self) -> DeviceClass;
    fn id(&self) -> DeviceId;
    fn read(&self, pos: usize, buf: &mut [u8]) -> Result<usize, ErrorKind>;
    fn write(&self, pos: usize, buf: &[u8]) -> Result<usize, ErrorKind>;
    fn open(&self, oflag: i32) -> Result<(), ErrorKind>;
    fn close(&self) -> Result<(), ErrorKind>;
    fn ioctl(&self, _request: u32, _arg: usize) -> Result<(), ErrorKind> {
        Err(ErrorKind::Unsupported)
    }
}

static DEVICE_MANAGER: Once<DeviceManager> = Once::new();

pub struct DeviceManager {
    devices: SpinRwLock<BTreeMap<&'static str, Arc<dyn Device>>>,
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
        F: Fn(Arc<dyn Device>),
    {
        for device in self.devices.read().values() {
            callback(device.clone());
        }
        Ok(())
    }
}
