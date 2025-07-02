use crate::{allocator, error::Error, vfs::procfs::ProcFileOps};
use alloc::{format, string::String, vec::Vec};
use core::fmt::Write;

pub(crate) struct MemoryInfo;

impl ProcFileOps for MemoryInfo {
    fn get_content(&self) -> Result<Vec<u8>, Error> {
        let meminfo = allocator::memory_info();
        let available = meminfo.total - meminfo.used;
        // Pre-allocate buffer with estimated size
        let mut result = String::with_capacity(128);
        writeln!(result, "{:<14}{:>8} kB", "MemTotal:", meminfo.total / 1024).unwrap();
        writeln!(result, "{:<14}{:>8} kB", "MemAvailable:", available / 1024).unwrap();
        writeln!(result, "{:<14}{:>8} kB", "MemUsed:", meminfo.used / 1024).unwrap();
        writeln!(
            result,
            "{:<14}{:>8} kB",
            "MemMaxUsed:",
            meminfo.max_used / 1024
        )
        .unwrap();
        Ok(result.as_bytes().to_vec())
    }

    fn set_content(&self, content: Vec<u8>) -> Result<usize, Error> {
        Ok(0)
    }
}
