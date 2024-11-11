/// arch Core Apis
pub trait ICore {
    /// new
    fn new() -> Self;
    /// Start peripherals used by kernel.
    fn start(&mut self);
}
