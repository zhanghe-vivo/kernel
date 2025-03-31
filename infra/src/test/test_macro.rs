/// Rust unstable custom test framework test macro
///
/// Example:
/// ```rust
/// use bluekernel_infra::custom_test;
///
/// custom_test! {
///     fn test_func() {
///         // Testing logic and asserts
///     }
/// }
/// ```
/// NOTE: User should redefine the println! macro to his own to ensure it outputs properly
/// ```rust
/// use <crate-name>::println;
///```
#[macro_export]
macro_rules! custom_test {
    (fn $name:ident($($arg:ident: $t:ty),*) $body:block) => {
        #[test_case]
        fn $name($($arg: $t),*) {
            println!("[Test case]: [{}]", stringify!($name));
            $body
            println!("[ok]");
        }
    };
}