use crate::drivers::device::{Device, DeviceBase, DeviceClass, DeviceFlags, DeviceId};
use alloc::sync::Arc;
use delegate::delegate;
use embedded_io::ErrorKind;

pub struct Null {
    base: DeviceBase,
}

impl Null {
    pub fn new() -> Self {
        Self {
            base: DeviceBase::new("null", DeviceClass::Char, DeviceFlags::RDWR),
        }
    }

    pub fn register() -> Result<(), ErrorKind> {
        let null = Arc::new(Null::new());
        crate::drivers::device::DeviceManager::get().register_device("/dev/null", null)
    }

    delegate! {
        to self.base {
            fn flags(&self) -> DeviceFlags;
            fn check_flags(&self, oflag: i32) -> Result<(), ErrorKind>;
            fn set_oflag(&self, oflag: i32);
            fn oflag(&self) -> i32;
            fn is_blocking(&self) -> bool;
            fn inc_open_count(&self) -> u32;
            fn dec_open_count(&self) -> u32;
            fn is_opened(&self) -> bool;
        }
    }
}

impl Device for Null {
    delegate! {
        to self.base {
            fn name(&self) -> &'static str;
            fn class(&self) -> DeviceClass;
        }
    }

    fn id(&self) -> DeviceId {
        DeviceId {
            major: 1, // 1 is the major number for char devices
            minor: 3, // 3 is the minor number for /dev/null
        }
    }

    fn read(&self, _pos: usize, _buf: &mut [u8]) -> Result<usize, ErrorKind> {
        // Always return EOF (0 bytes read)
        Ok(0)
    }

    fn write(&self, _pos: usize, buf: &[u8]) -> Result<usize, ErrorKind> {
        // Always succeed, but discard the data
        Ok(buf.len())
    }

    fn ioctl(&self, request: u32, arg: usize) -> Result<(), ErrorKind> {
        return Err(ErrorKind::Unsupported);
    }

    fn open(&self, oflag: i32) -> Result<(), ErrorKind> {
        self.check_flags(oflag)?;
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
    use crate::drivers::device::DeviceManager;
    use bluekernel_test_macro::test;

    #[test]
    fn test_null_device_read() {
        let null = Null::new();
        let mut buffer = [0u8; 10];

        // Read should always return 0 bytes (EOF)
        let result = null.read(0, &mut buffer);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn test_null_device_write() {
        let null = Null::new();
        let buffer = [1u8, 2, 3, 4, 5];

        // Write should always succeed and return the buffer length
        let result = null.write(0, &buffer);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), buffer.len());
    }

    #[test]
    fn test_null_device_open_close() {
        let null = Null::new();

        // Test opening with valid flags
        let result = null.open(0);
        assert!(result.is_ok());
        assert!(null.is_opened());

        // Test closing
        let result = null.close();
        assert!(result.is_ok());
        assert!(!null.is_opened());
    }

    #[test]
    fn test_null_device_id() {
        let null = Null::new();
        let id = null.id();

        assert_eq!(id.major, 1);
        assert_eq!(id.minor, 3);
    }

    #[test]
    fn test_null_device_registration() {
        // Verify we can find the device
        let device = DeviceManager::get().find_device("/dev/null");
        assert!(device.is_ok());
    }
}
