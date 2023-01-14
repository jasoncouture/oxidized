use std::process::Command;

fn main() {
    // read env variables that were set in build script
    let uefi_path = env!("UEFI_PATH");
    let bios_path = env!("BIOS_PATH");

    // choose whether to start the UEFI or BIOS image
    const UEFI: bool = true;
    let image = match UEFI {
        true => uefi_path,
        false => bios_path,
    };

    let mut cmd = create_command(image, UEFI);
    println!("Starting image {} with qemu", image);
    let mut child = cmd.spawn().expect("Unable to spawn qemu process");
    child.wait().expect("Unable to wait for child exit!");
}

fn create_command(image_path: &str, uefi: bool) -> Command {
    let mut cmd = std::process::Command::new("qemu-system-x86_64");

    if uefi {
        cmd.arg("-bios").arg(ovmf_prebuilt::ovmf_pure_efi());
    }

    cmd.arg("-drive")
        .arg(format!("format=raw,file={image_path}"))
        .arg("-serial")
        .arg("stdio")
        .arg("-m")
        .arg("size=1024")
        .arg("-smp")
        .arg("cpus=2")
        .arg("-d")
        .arg("cpu_reset")
        .arg("-accel")
        .arg("tcg");

    return cmd;
}
