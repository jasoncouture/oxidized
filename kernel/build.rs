// build.rs

use std::collections::HashMap;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use walkdir::WalkDir;

fn main() {
    create_metadata_constants();
    build_assembly();
}

fn build_assembly() {
    let build_path = env::var("CARGO_MANIFEST_DIR").unwrap();
    let base_path = Path::new(build_path.as_str())
        .canonicalize()
        .expect("Failed to cannoicalize .");
    let borrowed_base_path = &base_path;
    for f in WalkDir::new(borrowed_base_path)
        .same_file_system(true)
        .follow_links(true)
        .into_iter()
        .filter_map(|f| f.ok())
        .filter(|f| f.file_type().is_file())
        .filter(|f| match f.path().extension() {
            Some(s) => match s.to_str() {
                Some(str) => match str {
                    "nasm" => true,
                    "s" => true,
                    "asm" => true,
                    _ => false,
                },
                _ => false,
            },
            _ => false,
        })
    {
        println!("Processing source file {}", f.path().to_str().unwrap());
        let file_path = f.path();
        build_assembly_file(file_path, borrowed_base_path);
    }
}

fn build_assembly_file(file: &Path, base_path: &PathBuf) -> PathBuf {
    let out_dir = env::var("OUT_DIR").unwrap();
    let relative = match file.strip_prefix(base_path) {
        Ok(f) => f,
        Err(_) => panic!(
            "File {} is not relative to {}",
            file.to_str().unwrap(),
            base_path.to_str().unwrap()
        ),
    };

    println!("cargo:rerun-if-changed={}", relative.to_str().unwrap());

    let output_relative = relative.with_extension("bin");

    let output_file = format!(
        "{}/{}",
        out_dir,
        match file.is_absolute() {
            true => output_relative.to_str().unwrap(),
            false => output_relative.file_name().unwrap().to_str().unwrap(),
        }
    );
    let output_file = output_file.as_str();
    let output_file = Path::new(output_file);
    let output_file = output_file.to_path_buf();
    if let Some(p) = output_file.parent() {
        println!("Ensuring path exists: {}", p.to_str().unwrap());
        let result = fs::create_dir_all(p);
        if result.is_err() {
            panic!(
                "Failed to create base path for output: {}",
                p.to_str().unwrap()
            )
        }
    };

    let output_file = output_file.to_str().unwrap();

    let status = Command::new("nasm")
        .arg("-f")
        .arg("bin")
        .arg("-o")
        .arg(output_file)
        .arg(file)
        .status()
        .expect("failed to run nasm");

    let status_code = match status.code() {
        Some(c) => c,
        None => panic!(
            "Could not get nasm process exit code while building {}",
            relative.to_str().unwrap()
        ),
    };
    if status_code != 0 {
        panic!("Failed to assemble {}", relative.to_str().unwrap());
    }

    Path::new(output_file).to_path_buf()
}

fn create_metadata_constants() {
    const CONSTANT_PREFIX: &str = "METADATA_";
    let constant_map = HashMap::from([
        ("CARGO_CFG_TARGET_ARCH", "BUILD_ARCH"),
        ("TARGET", "BUILD_TARGET"),
        ("PROFILE", "PROFILE"),
    ]);
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("metadata_constants.rs");
    let mut constants: String = String::from("");
    for (key, value) in constant_map {
        let last = constants.clone();
        let os_value = env_val(key);
        constants = std::format!(
            "{}pub const {}{}: &str = \"{}\";\n",
            last,
            CONSTANT_PREFIX,
            value,
            os_value.to_string_lossy()
        );
    }
    constants = std::format!(
        "{}const {}VERSION: Option<&str> = option_env!(\"CARGO_PKG_VERSION\");\n",
        constants,
        CONSTANT_PREFIX
    );
    fs::write(&dest_path, constants).unwrap();

    println!("cargo:rerun-if-changed=build.rs");
}

fn env_val(name: &str) -> OsString {
    match env::var_os(name) {
        Some(val) => return val,
        None => return OsString::from(""),
    };
}
