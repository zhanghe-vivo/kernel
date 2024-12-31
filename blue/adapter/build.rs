use enum_iterator::all;
use std::env;

fn main() {
    if let Ok(os_adapter) = env::var("OS_ADAPTER") {
        if os_adapter == "rt_thread" {
            println!("cargo:rustc-cfg=rt_thread");
            println!("cargo:rustc-check-cfg=cfg(rt_thread)");
        }
    }

    for feature in all::<blue_kconfig::Feature>() {
        if feature.is_enabled() {
            println!("cargo:rustc-cfg=feature=\"{}\"", feature.to_string());
        }
    }
}
