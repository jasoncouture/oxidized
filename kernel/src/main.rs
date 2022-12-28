#![no_std]
#![no_main]
#![feature(const_mut_refs)]
#![feature(custom_test_frameworks)]
#![feature(slice_pattern)]
#![feature(alloc_error_handler)]
#![feature(abi_x86_interrupt)]
#![feature(asm_const)]
//#[cfg_attr(target_arch = "x86_64")]
#![test_runner(crate::test_runner::test_runner)]
#![reexport_test_harness_main = "test_main"]
include!(concat!(env!("OUT_DIR"), "/metadata_constants.rs"));
extern crate alloc;
pub(crate) mod console;
pub(crate) mod framebuffer;
pub(crate) mod interrupts;
pub(crate) mod logging;

mod loader;
mod memory;
mod panic;
pub(crate) mod serial;
mod test_runner;
pub mod thread;
mod unit_tests;
mod acpi;

use bootloader_api::{
    config::{self, Mapping},
    info::MemoryRegionKind,
};
use framebuffer::*;
use memory::{allocator::KERNEL_FRAME_ALLOCATOR, *};
use x86_64::VirtAddr;
use x86_64::software_interrupt;
use core::arch::asm;
const CONFIG: bootloader_api::BootloaderConfig = {
    let mut config = bootloader_api::BootloaderConfig::new_default();
    config.kernel_stack_size = 1024 * 1024; // 1MiB
    config.mappings.aslr = true;
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    //config.mappings.framebuffer = Mapping::Dynamic;
    config
};

bootloader_api::entry_point!(kernel_boot, config = &CONFIG);

#[allow(unreachable_code)]
fn kernel_boot(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    early_init(boot_info);
    test_hook();
    verbose!(
        "Oxidized kernel v{}, starting",
        METADATA_VERSION.unwrap_or("Unknown")
    );
    kernel_main();
    panic!("Kernel exited, this should not happen!");
}

#[inline]
fn early_init(boot_info: &'static mut bootloader_api::BootInfo) {
    initialize_virtual_memory(
        VirtAddr::new(
            boot_info
                .physical_memory_offset
                .into_option()
                .expect("No physical memory offset available!"),
        ),
        &boot_info.memory_regions,
    );
    debug!("Setting up framebuffer");
    let fb_option: Option<&'static mut bootloader_api::info::FrameBuffer> =
        boot_info.framebuffer.as_mut();
    init_framebuffer(fb_option);
    clear();
    debug!("Reading ACPI");
    crate::acpi::init(boot_info.rsdp_addr.into_option());
    debug!("Initializing interrupts on CPU 0");
    interrupts::init();
}

fn clear() {
    let frame_buffer_wrapper = FRAME_BUFFER.lock();
    let frame_buffer = frame_buffer_wrapper.get_framebuffer().unwrap();
    frame_buffer.clear(&Color::black());
}

fn kernel_main() {
    unsafe {
        software_interrupt!(0x80);
        software_interrupt!(0x81);
    }
    verbose!("Bootloader provided memory map, with unusable page ranges:");
    unsafe {
        for range in KERNEL_FRAME_ALLOCATOR
            .get_memory_regions()
            .iter()
            .filter(|range| range.kind != MemoryRegionKind::Usable)
        {
            verbose!(
                "-- {:#016x} to {:#016x} - {:?}",
                range.start,
                range.end,
                range.kind
            );
        }
    }
}

fn test_hook() {
    #[cfg(test)]
    test_main();
}
