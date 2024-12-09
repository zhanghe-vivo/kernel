use bindgen::Builder;
use std::{env, fs, path::PathBuf};

fn main() {
    let current_dir = env::current_dir().expect("Failed to get current directory");
    let target_build_include_path =
        env::var("INCLUDE_PATH").expect("Failed to get target build include path");
    let target_build_include_paths: Vec<&str> = target_build_include_path.split(';').collect();
    let mut include_paths = vec![];
    for path in target_build_include_paths {
        let full_path = current_dir.join(path);
        include_paths.push(full_path);
    }

    let compat_os = env::var("COMPAT_OS").expect("Failed to get target build os");

    if compat_os == "rt_thread" {
        // bindgen
        let wrapper_header = current_dir.join("./include/rt_wrapper.h");
        println!(
            "cargo:rerun-if-changed={}",
            wrapper_header.to_string_lossy()
        );
        println!(
            "cargo:rerun-if-changed={}",
            current_dir
                .join("../../../include/rtconfig.h")
                .to_string_lossy()
        );
        println!(
            "cargo:rerun-if-changed={}",
            current_dir
                .join("../../../include/rtdef.h")
                .to_string_lossy()
        );
        println!(
            "cargo:rerun-if-changed={}",
            current_dir
                .join("../../../include/rtthread.h")
                .to_string_lossy()
        );
        // FIXME
        fs::copy(
            "../../kernel/include/rust_wrapper.inc",
            "./include/rust_wrapper.inc",
        )
        .expect("Unable to copy file");
        include_paths.push(current_dir.join("./include"));

        let mut builder = Builder::default();
        for path in &include_paths {
            builder = builder.clang_arg(format!("-I{}", path.to_string_lossy()));
        }

        let bindings = builder
            .header(wrapper_header.to_string_lossy())
            .ctypes_prefix("core::ffi")
            .use_core()
            .generate()
            .expect("Unable to generate bindings");

        let out_path = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
        bindings
            .write_to_file(out_path.join("bindings.rs"))
            .expect("Couldn't write bindings!");
        println!("binfings.rs generated.");
    }
}
