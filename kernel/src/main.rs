#![no_std]
#![no_main]
#![feature(const_mut_refs)]
#![feature(custom_test_frameworks)]
#![feature(slice_pattern)]
#![feature(alloc_error_handler)]
#![feature(abi_x86_interrupt)]
#![feature(asm_const)]
#![feature(box_syntax)]
#![feature(once_cell)]
#![feature(naked_functions)]
#![feature(pointer_byte_offsets)]
#![feature(core_intrinsics)]
#![feature(pointer_is_aligned)]
//#[cfg_attr(target_arch = "x86_64")]
#![test_runner(crate::test_runner::test_runner)]
#![reexport_test_harness_main = "test_main"]
extern crate alloc;

use core::arch::asm;
use core::ptr::NonNull;

use bootloader_api::{config::Mapping, info::MemoryRegionKind, BootInfo};
use x86_64::{software_interrupt, VirtAddr};

use framebuffer::*;
use memory::{allocator::{KERNEL_FRAME_ALLOCATOR, KERNEL_HEAP_START, PAGE_SIZE}, *};
use thread::process::{process_manager, ProcessDescriptor, ProcessManager};

use crate::{
    arch::{
        arch_x86_64::{get_cpu_brand_string, get_cpu_vendor_string},
        enable_interrupts, get_current_cpu, get_timer_ticks, wait_for_interrupt,
    },
    serial::SERIAL1,
    thread::context::CONTEXTS,
};

include!(concat!(env!("OUT_DIR"), "/metadata_constants.rs"));
pub(crate) mod arch;
pub(crate) mod console;
pub(crate) mod framebuffer;
pub(crate) mod logging;

mod loader;
mod memory;
mod panic;
pub(crate) mod serial;
mod test_runner;
pub mod thread;
mod unit_tests;

const CONFIG: bootloader_api::BootloaderConfig = {
    let mut config = bootloader_api::BootloaderConfig::new_default();
    config.mappings.aslr = true;
    config.kernel_stack_size = 1024 * 256; // 256k
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config.mappings.dynamic_range_end = Some(KERNEL_HEAP_START as u64);
    config.mappings.dynamic_range_start = Some((PAGE_SIZE * 100) as u64); // Reserve the first 1mb of virtual address space. Please.
    config
};

bootloader_api::entry_point!(kernel_boot, config = &CONFIG);
static mut BOOT_INFO: Option<NonNull<BootInfo>> = None;

#[allow(unreachable_code)]
fn kernel_boot(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    println!("Booting");
    unsafe {
        println!("Creating boot info pointer");
        BOOT_INFO = NonNull::new(boot_info);
        println!("Starting early init");
        early_init(BOOT_INFO.unwrap().as_mut());
        hardware_init(BOOT_INFO.unwrap().as_mut());
    }
    kernel_main();
    panic!("Kernel exited, this should not happen!");
}

#[inline]
fn early_init(boot_info: &'static mut BootInfo) {
    println!("Initializing virtual memory");
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
}

fn hardware_init(boot_info: &BootInfo) {
    let cpu = get_current_cpu();
    debug!("Initializing hardware on boot CPU (ACPI ID: {})", cpu);
    arch::init(boot_info);
}

fn clear() {
    let frame_buffer_wrapper = FRAME_BUFFER.lock();
    let frame_buffer = frame_buffer_wrapper.get_framebuffer().unwrap();
    frame_buffer.clear(&Color::black());
}

fn kernel_main() {
    verbose!(
        "Oxidized kernel v{}, starting",
        METADATA_VERSION.unwrap_or("Unknown")
    );
    verbose!("CPU Vendor: {}", get_cpu_vendor_string());
    verbose!("CPU Brand : {}", get_cpu_brand_string());
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
    create_kernel_process();
    enable_interrupts();
    let mut online_cpus = 0;
    let status_bits = arch::arch_x86_64::cpu::get_cpu_status_bits();
    {
        online_cpus = status_bits.lock().iter().filter(|b| *b == true).count();
    }
    debug!("Boot complete with {} CPUs online.", online_cpus);
    debug!("TODO: Implement something.");
    // debug!("Requesting context switch");
    // unsafe {
    //     software_interrupt!(254);
    // }
    // debug!("Execution resumed after context switch!");
    loop {
        
        // let ticks = get_timer_ticks();
        // debug!("Tick: {}", ticks);
        wait_for_interrupt();
    }
}

fn create_kernel_process() {
    let manager = process_manager();
    let process = manager.create_process();
    debug!(
        "Created kernel process \"{}\" with control group: {}",
        process.get_id(),
        process.get_control_group()
    );
}
