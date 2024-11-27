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

#[macro_export]
macro_rules! function {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            core::intrinsics::type_name::<T>()
        }
        let name = type_name_of(f);
        &name[6..name.len() - 4]
    }};
}

// https://stackoverflow.com/questions/54999851/how-do-i-get-the-return-address-of-a-function
// https://github.com/rust-lang/rust/issues/29602
// need to add #![feature(link_llvm_intrinsics)]
extern "C" {
    #[allow(dead_code)]
    #[link_name = "llvm.returnaddress"]
    fn return_address(a: i32) -> *const u8;
}

#[macro_export]
macro_rules! caller_address {
    () => {
        unsafe { return_address(0) }
    };
}

#[macro_export]
macro_rules! container_of {
    ($ptr:expr, $type:path, $field:ident) => {
        $ptr.cast::<u8>()
            .sub(core::mem::offset_of!($type, $field))
            .cast::<$type>()
    };
}

/// Get the struct for this entry.
#[macro_export]
macro_rules! list_head_entry {
    ($node:expr, $type:ty, $($f:tt)*) => {
        crate::container_of!($node, $type, $($f)*)
    };
}

/// Iterate over a doubly linked list.
#[macro_export]
macro_rules! list_head_for_each {
    ($pos:ident, $head:expr, $code:block) => {
        let mut $pos = $head;
        while let Some(next) = $pos.next() {
            $pos = unsafe { &*next.as_ptr() };
            if core::ptr::eq($pos, $head) {
                break;
            }
            $code
        }
    };
}
