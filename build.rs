use std::path::Path;

const UEFI_IMAGE_NAME: &str = "uefi.img";

fn main() {
    // set by cargo, build scripts should use this directory for output files
    let kernel_os_string = std::env::var_os("CARGO_BIN_FILE_KERNEL_kernel").unwrap();
    let kernel_str = kernel_os_string.to_str().unwrap();
    let ramdisk = Path::new("build.rs").canonicalize().unwrap();

    let out_dir_os_string = std::env::var_os("OUT_DIR").unwrap();
    let out_dir_str = out_dir_os_string.to_str().unwrap();
    let out_dir = Path::new(out_dir_str);
    // set by cargo's artifact dependency feature, see
    // https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#artifact-dependencies
    let kernel = Path::new(kernel_str);

    let uefi_path = out_dir.join(UEFI_IMAGE_NAME);
    let binding = kernel.to_path_buf();
    let mut disk_image_builder = bootloader::DiskImageBuilder::new(binding);
    disk_image_builder.set_ramdisk(ramdisk);
    disk_image_builder.create_uefi_image(&uefi_path).unwrap();

    println!("cargo:rustc-env=UEFI_PATH={}", uefi_path.display());
}
