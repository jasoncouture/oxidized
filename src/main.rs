use std::process::Command;

fn main() {
    // read env variables that were set in build script
    let uefi_path = env!("UEFI_PATH");

    let mut cmd = create_command(uefi_path);
    println!("Starting image {} with qemu", uefi_path);
    let mut child = cmd.spawn().expect("Unable to spawn qemu process");
    child.wait().expect("Unable to wait for child exit!");
}

fn create_command(image_path: &str) -> Command {
    let mut cmd = std::process::Command::new("qemu-system-x86_64");

    cmd.arg("-bios").arg(ovmf_prebuilt::ovmf_pure_efi());

    cmd.arg("-drive")
        .arg(format!("format=raw,file={image_path}"))
        .arg("-serial")
        .arg("stdio")
        .arg("-m")
        .arg("size=2048")
        .arg("-smp")
        .arg("cpus=4")
        .arg("-d")
        .arg("cpu_reset")
        .arg("-accel")
        .arg("kvm")
        .arg("-display")
        .arg("none");

    return cmd;
}
