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
use kernel_vga_buffer::{println, WRITER, Color};
use pal::HardwareControl;
use pal_x86_64::PAL_PLATFORM;
#[no_mangle]
pub extern "C" fn _start() {
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
    WRITER.lock().set_foreground_color(Color::Red);
    println!("Oxidized kernel");
    WRITER.lock().set_foreground_color(Color::White);
    println!("Version     : {}", METADATA_VERSION.unwrap_or("unknown"));
    println!("Architecture: {}", METADATA_BUILD_ARCH);
    println!("Compiler    : {}", METADATA_BUILD_TARGET);
    WRITER.lock().set_foreground_color(Color::LightGreen);
}

fn kernel_main() {
    PAL_PLATFORM.halt();
}


