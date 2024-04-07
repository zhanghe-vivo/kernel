// https://stackoverflow.com/questions/54999851/how-do-i-get-the-return-address-of-a-function
// https://github.com/rust-lang/rust/issues/29602
// need to add #![feature(link_llvm_intrinsics)]

#![feature(link_llvm_intrinsics)]
extern {
    #[link_name = "llvm.returnaddress"]
    fn return_address(a: i32) -> *const u8;
}

macro_rules! caller_address {
    () => {
        unsafe { return_address(0) }
    };
}
