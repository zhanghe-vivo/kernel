#[const_trait]
pub trait Adapter {
    fn offset() -> usize;
}

#[macro_export]
macro_rules! impl_simple_intrusive_adapter {
    ($name:ident, $ty:ty, $($fields:expr)+) => {
        #[derive(Default, Debug)]
        pub struct $name;
        impl $crate::intrusive::Adapter for $name {
            fn offset() -> usize {
                core::mem::offset_of!($ty, $($fields)+)
            }
        }
    }
}
