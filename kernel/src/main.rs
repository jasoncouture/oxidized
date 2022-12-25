#![no_std]
#![no_main]
#![feature(const_mut_refs)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner::test_runner)]
#![reexport_test_harness_main = "test_main"]
include!(concat!(env!("OUT_DIR"), "/metadata_constants.rs"));
pub(crate) mod console;
pub(crate) mod framebuffer;
mod panic;
mod test_runner;
mod unit_tests;

use core::option;

use bootloader_api::config::Mapping;
use console::*;
use framebuffer::*;
use lazy_static::*;

const CONFIG: bootloader_api::BootloaderConfig = {
    let mut config = bootloader_api::BootloaderConfig::new_default();
    config.kernel_stack_size = 100 * 1024; // 100 KiB
    config.mappings.aslr = true;
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config.mappings.framebuffer = Mapping::Dynamic;
    config
};

bootloader_api::entry_point!(kernel_boot, config = &CONFIG);

#[allow(unreachable_code)]
fn kernel_boot(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    early_init(boot_info);
    test_hook();
    kernel_main();
    loop {
        x86_64::instructions::interrupts::disable();
        x86_64::instructions::hlt();
    }
}

lazy_static! {
    static ref FONT: Font = Font::new();
}

fn early_init(boot_info: &'static mut bootloader_api::BootInfo) {
    let fb_option: Option<&'static mut bootloader_api::info::FrameBuffer> =
        boot_info.framebuffer.as_mut();
    init_framebuffer(fb_option);
    clear();
}

fn clear() {
    let frame_buffer_wrapper = FRAME_BUFFER.lock();
    let frame_buffer = frame_buffer_wrapper.get_framebuffer().unwrap();
    frame_buffer.clear(&Color::black());
}

fn kernel_main() {}

fn test_hook() {
    #[cfg(test)]
    test_main();
}
