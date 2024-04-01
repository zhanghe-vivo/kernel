use bindgen;
use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=rt_wrapper.h");

    // 从环境变量中获取 CPPPATH, TODO
    // let cpppath = env::var("CPPPATH").unwrap_or_default();

    let current_dir = env::current_dir().expect("Failed to get current directory");
    let target_build_include_path = env::var("INCLUDE_PATH").expect("Failed to get target build include path");
    let include_path = vec![current_dir.join("../include"), 
                            current_dir.join("../components/drivers/include"),
                            current_dir.join("../components/finsh"),
                            current_dir.join(target_build_include_path),
                            current_dir.join("../libcpu/arm/cortex-a"),
                            current_dir.join("../components/legacy")];

    let mut builder = bindgen::Builder::default();
    for path in &include_path {
        builder = builder.clang_arg(format!("-I{}", path.to_string_lossy()));
    }

    let bindings = builder
        .header("rt_wrapper.h")
        .ctypes_prefix("cty")
        .use_core()
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
    println!("binfings.rs generated.");
}
