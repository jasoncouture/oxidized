// build.rs

use std::collections::HashMap;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::Path;

fn main() {
    const CONSTANT_PREFIX: &str = "METADATA_";
    let constant_map = HashMap::from([
        ("CARGO_CFG_TARGET_ARCH", "BUILD_ARCH"), 
        ("TARGET", "BUILD_TARGET"), 
        ("PROFILE", "PROFILE")
        ]
    );
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("metadata_constants.rs");
    let mut constants : String = String::from("");
    for (key, value) in constant_map {
        let last = constants.clone();
        let os_value = env_val(key);
        constants = std::format!("{}pub const {}{}: &str = \"{}\";\n", last, CONSTANT_PREFIX, value, os_value.to_string_lossy());
    }
    constants = std::format!("{}const {}VERSION: Option<&str> = option_env!(\"CARGO_PKG_VERSION\");\n", constants, CONSTANT_PREFIX);
    fs::write(
        &dest_path,
        constants
    ).unwrap();
    println!("cargo:rerun-if-changed=build.rs");
}


fn env_val(name: &str) -> OsString {
    match env::var_os(name) {
        Some(val) => return val,
        None => return OsString::from("")
    };
}
