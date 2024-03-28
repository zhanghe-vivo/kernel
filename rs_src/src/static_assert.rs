#[macro_export]
macro_rules! static_assert {
    ($condition:expr) => {
        // Based on the latest one in `rustc`'s one before it was [removed].
        //
        // [removed]: https://github.com/rust-lang/rust/commit/c2dad1c6b9f9636198d7c561b47a2974f5103f6d
        #[allow(dead_code)]
        const _: () = [()][!($condition) as usize];
    };
}
