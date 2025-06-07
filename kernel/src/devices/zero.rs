use crate::{
    devices::{Device, DeviceBase, DeviceClass, DeviceId, DeviceManager},
    vfs::vfs_mode::AccessMode,
};
use alloc::sync::Arc;
use delegate::delegate;
use embedded_io::ErrorKind;

pub struct Zero {
    base: DeviceBase,
}

impl Zero {
    pub fn new() -> Self {
        Self {
            base: DeviceBase::new("zero", DeviceClass::Char, AccessMode::O_RDWR),
        }
    }

    pub fn register() -> Result<(), ErrorKind> {
        let zero = Arc::new(Zero::new());
        DeviceManager::get().register_device("/dev/zero", zero)
    }

    delegate! {
        to self.base {
            fn check_permission(&self, oflag: i32) -> Result<(), ErrorKind>;
            fn inc_open_count(&self) -> u32;
            fn dec_open_count(&self) -> u32;
            fn is_opened(&self) -> bool;
        }
    }
}

impl Device for Zero {
    delegate! {
        to self.base {
            fn name(&self) -> &'static str;
            fn class(&self) -> DeviceClass;
            fn access_mode(&self) -> AccessMode;
        }
    }

    fn id(&self) -> DeviceId {
        DeviceId {
            major: 1, // 1 is the major number for char devices
            minor: 5, // 5 is the minor number for /dev/zero
        }
    }

    fn read(&self, _pos: usize, buf: &mut [u8], _is_blocking: bool) -> Result<usize, ErrorKind> {
        // Fill buffer with zeros
        buf.fill(0);
        Ok(buf.len())
    }

    fn write(&self, _pos: usize, buf: &[u8], _is_blocking: bool) -> Result<usize, ErrorKind> {
        // Always succeed, but discard the data
        Ok(buf.len())
    }

    fn open(&self, oflag: i32) -> Result<(), ErrorKind> {
        self.check_permission(oflag)?;
        self.inc_open_count();
        Ok(())
    }

    fn close(&self) -> Result<(), ErrorKind> {
        self.dec_open_count();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::DeviceManager;
    use bluekernel_test_macro::test;

    #[test]
    fn test_zero_device_read() {
        let zero = Zero::new();
        let mut buffer = [1u8; 10];

        // Read should fill buffer with zeros
        let result = zero.read(0, &mut buffer, true);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), buffer.len());
        assert!(buffer.iter().all(|&x| x == 0));
    }

    #[test]
    fn test_zero_device_write() {
        let zero = Zero::new();
        let buffer = [1u8, 2, 3, 4, 5];

        // Write should always succeed and return the buffer length
        let result = zero.write(0, &buffer, true);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), buffer.len());
    }

    #[test]
    fn test_zero_device_open_close() {
        let zero = Zero::new();

        // Test opening with valid flags
        let result = zero.open(libc::O_RDWR);
        assert!(result.is_ok());
        assert!(zero.is_opened());

        // Test closing
        let result = zero.close();
        assert!(result.is_ok());
        assert!(!zero.is_opened());
    }

    #[test]
    fn test_zero_device_id() {
        let zero = Zero::new();
        let id = zero.id();

        assert_eq!(id.major, 1);
        assert_eq!(id.minor, 5);
    }

    #[test]
    fn test_zero_device_registration() {
        // Verify we can find the device
        let device = DeviceManager::get().find_device("/dev/zero");
        assert!(device.is_ok());
    }
}
