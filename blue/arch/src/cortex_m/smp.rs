/// cortex-m not support smp, always return 0
/// Return the executing core's id.
#[inline(always)]
pub fn core_id<T>() -> T
where
    T: From<u8>,
{
    T::from(0u8)
}
