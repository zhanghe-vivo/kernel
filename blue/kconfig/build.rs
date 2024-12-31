// build.rs
fn main() {
    println!("cargo:rerun-if-changed=kconfig/config");
}
