use bindgen::Builder;
use std::{env, path::PathBuf};

fn main() {
    let board = env::var("BOARD").unwrap();

    println!(
        "cargo:rustc-check-cfg=cfg(target_board,values(\"qemu_mps2_an385\",\"qemu_mps3_an547\"))"
    );

    if board == "qemu-mps2-an385" {
        println!("cargo:rustc-cfg=target_board=\"qemu_mps2_an385\"");
    } else if board == "qemu-mps3-an547" {
        println!("cargo:rustc-cfg=target_board=\"qemu_mps3_an547\"");
    }

    // add bindgen for serial device
    let current_dir = env::current_dir().expect("Failed to get current directory");
    let target_build_include_path =
        env::var("INCLUDE_PATH").expect("Failed to get target build include path");
    let target_build_include_paths: Vec<&str> = target_build_include_path.split(';').collect();
    let mut include_paths = vec![];
    for path in target_build_include_paths {
        let full_path = current_dir.join(path);
        include_paths.push(full_path);
    }
    include_paths.push(current_dir.join("../kernel/include"));
    include_paths.push(current_dir.join("../../components/drivers/include"));

    let adapt_os = env::var("OS_ADAPTER").expect("Failed to get target build os");

    if adapt_os == "rt_thread" {
        include_paths.push(current_dir.join("../adapter/rt_thread/include"));
        // bindgen
        let wrapper_header = current_dir.join("./include/rt_wrapper.h");
        println!(
            "cargo:rerun-if-changed={}",
            wrapper_header.to_string_lossy()
        );

        println!(
            "cargo:rerun-if-changed={}",
            current_dir
                .join("../kernel/include/rust_wrapper.inc")
                .to_string_lossy()
        );
        println!(
            "cargo:rerun-if-changed={}",
            current_dir
                .join("../adapter/rt_thread/include/rt_rust_wrapper.inc")
                .to_string_lossy()
        );
        println!(
            "cargo:rerun-if-changed={}",
            current_dir.join("../../include/rtdef.h").to_string_lossy()
        );
        println!(
            "cargo:rerun-if-changed={}",
            current_dir
                .join("../../include/rtthread.h")
                .to_string_lossy()
        );
        println!(
            "cargo:rerun-if-changed={}",
            current_dir
                .join("../../components/drivers/include/rtdevice.h")
                .to_string_lossy()
        );

        let mut builder = Builder::default();
        for path in &include_paths {
            builder = builder.clang_arg(format!("-I{}", path.to_string_lossy()));
        }

        let bindings = builder
            .header(wrapper_header.to_string_lossy())
            .ctypes_prefix("core::ffi")
            .use_core()
            .clang_arg("-fshort-enums") // https://github.com/rust-lang/rust-bindgen/issues/711
            .generate()
            .expect("Unable to generate bindings");

        let out_path = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
        bindings
            .write_to_file(out_path.join("src/bindings.rs"))
            .expect("Couldn't write bindings!");
        println!("binfings.rs generated.");
    }
}
