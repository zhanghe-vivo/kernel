#[cfg(not(cortex_a))]
pub mod console;
pub mod device;
mod error;
mod null;
pub mod serial;
mod zero;

use embedded_io::ErrorKind;
pub fn init() -> Result<(), ErrorKind> {
    null::Null::register()?;
    zero::Zero::register()?;
    Ok(())
}
