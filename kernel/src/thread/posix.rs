extern crate alloc;

use alloc::string::String;

#[derive(Debug)]
pub(crate) struct PosixCompat {
    pub cwd: String,
}
