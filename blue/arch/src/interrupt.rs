/// interrupt operations
pub trait IInterrupt {
    /// Disable any interrupt below priority.
    fn disable_interrupts() -> usize;
    /// Enable all interrupts.
    fn enable_interrupts(state: usize);
    fn is_interrupts_active() -> bool;
    /// is in interrupts
    fn is_in_interrupt() -> bool;
    /// reset cpu
    fn sys_reset() -> !;
}
