use bindgen::Builder;
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=rt_wrapper.h");

    // 从环境变量中获取 CPPPATH, TODO
    // let cpppath = env::var("CPPPATH").unwrap_or_default();

    let current_dir = env::current_dir().expect("Failed to get current directory");
    let target_build_include_path =
        env::var("INCLUDE_PATH").expect("Failed to get target build include path");
    let include_path = vec![
        current_dir.join("./include"),
        current_dir.join("../include"),
        current_dir.join("../components/drivers/include"),
        current_dir.join("../components/finsh"),
        current_dir.join("../components/legacy"),
        current_dir.join(target_build_include_path),
        current_dir.join("../libcpu/arm/cortex-a"),
    ];

    let mut config = cbindgen::Config::default();

    config.export.item_types = vec![cbindgen::ItemType::Structs, cbindgen::ItemType::Enums];

    config.export.exclude = vec!["ListHead".to_string()];

    let rename_list = HashMap::from([
        ("BaseObject".to_string(), "rt_object".to_string()),
        ("Stack".to_string(), "rt_stack".to_string()),
        ("RtThread".to_string(), "rt_thread".to_string()),
        ("Timer".to_string(), "rt_timer".to_string()),
        ("ListHead".to_string(), "rt_list_t".to_string()),
        (
            "ObjectInformation".to_string(),
            "rt_object_information".to_string(),
        ),
    ]);

    config.export.rename = rename_list;

    config.defines = HashMap::from([
        (
            "feature = RT_USING_SMP".to_string(),
            "RT_USING_SMP".to_string(),
        ),
        (
            "feature = RT_USING_MUTEX".to_string(),
            "RT_USING_MUTEX".to_string(),
        ),
        (
            "feature = RT_USING_EVENT".to_string(),
            "RT_USING_EVENT".to_string(),
        ),
        (
            "feature = RT_DEBUGING_SPINLOCK".to_string(),
            "RT_DEBUGING_SPINLOCK".to_string(),
        ),
    ]);

    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    cbindgen::Builder::new()
        .with_config(config)
        .with_crate(crate_dir)
        .with_no_includes()
        .with_language(cbindgen::Language::C)
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file("include/rust_wrapper.inc");

    let mut builder = Builder::default();
    for path in &include_path {
        builder = builder.clang_arg(format!("-I{}", path.to_string_lossy()));
    }

    let bindings = builder
        .header("rt_wrapper.h")
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
