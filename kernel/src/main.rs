#![no_std]
#![no_main]
#![feature(const_mut_refs)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner::test_runner)]
#![reexport_test_harness_main = "test_main"]
include!(concat!(env!("OUT_DIR"), "/metadata_constants.rs"));
mod test_runner;
mod panic;
mod unit_tests;
use pal::*;
use pal_x86_64::*;

const CONFIG: bootloader_api::BootloaderConfig = {
    let mut config = bootloader_api::BootloaderConfig::new_default();
    config.kernel_stack_size = 100 * 1024; // 100 KiB
    config.mappings.aslr = true;
    config
};

bootloader_api::entry_point!(kernel_boot, config = &CONFIG);


#[allow(unreachable_code)]
fn kernel_boot(_boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    early_init();
    test_hook();
    kernel_main();
    panic!("Kernel exited, this should never happen.");
}

fn test_hook() {
    #[cfg(test)]
    test_main();
}

fn early_init() {
    PAL_PLATFORM.init();
    println!("Oxidized kernel");
    println!("Version     : {}", METADATA_VERSION.unwrap_or("unknown"));
    println!("Architecture: {}", METADATA_BUILD_ARCH);
    println!("Compiler    : {}", METADATA_BUILD_TARGET);
}

fn kernel_main() -> ! {
    PAL_PLATFORM.halt();
}


