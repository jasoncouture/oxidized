use std::path::Path;
const UEFI_IMAGE_NAME: &str = "uefi.img";
const BIOS_IMAGE_NAME: &str = "bios.img";

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

    // create an UEFI disk image (optional)
    let uefi_path = out_dir.join(UEFI_IMAGE_NAME);
    bootloader::UefiBoot::new(&kernel)
        .set_ramdisk(&ramdisk)
        .create_disk_image(&uefi_path)
        .unwrap();

    // create a BIOS disk image (optional)
    let bios_path = out_dir.join(BIOS_IMAGE_NAME);
    bootloader::BiosBoot::new(&kernel)
        .set_ramdisk(&ramdisk)
        .create_disk_image(&bios_path)
        .unwrap();

    // pass the disk image paths as env variables to the `main.rs`
    println!("cargo:rustc-env=UEFI_PATH={}", uefi_path.display());
    println!("cargo:rustc-env=BIOS_PATH={}", bios_path.display());
}
