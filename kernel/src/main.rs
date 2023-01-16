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
#![feature(error_in_core)]
extern crate alloc;

use core::ptr::NonNull;

use alloc::{
    format,
    string::{String, ToString},
};
use arch::arch_x86_64::cpu::{cpu_apic_id, CPU_STACK_PAGES};
use bootloader_api::{config::Mapping, BootInfo};
use device::Device;
use spin::Mutex;
use uuid::Uuid;
use x86_64::VirtAddr;

use framebuffer::*;
use memory::{
    allocator::{KERNEL_HEAP_START, PAGE_SIZE},
    *,
};

use crate::{
    arch::{
        arch_x86_64::{get_cpu_brand_string, get_cpu_vendor_string},
        get_current_cpu, wait_for_interrupt,
    },
    device::get_mut_device_tree,
};

include!(concat!(env!("OUT_DIR"), "/metadata_constants.rs"));
pub(crate) mod arch;
pub(crate) mod console;
pub(crate) mod framebuffer;
pub(crate) mod logging;

mod device;
pub mod errors;
mod loader;
mod memory;
mod panic;
pub(crate) mod serial;
pub mod thread;

const CONFIG: bootloader_api::BootloaderConfig = {
    let mut config = bootloader_api::BootloaderConfig::new_default();
    config.mappings.aslr = true;
    config.kernel_stack_size = CPU_STACK_PAGES as u64 * PAGE_SIZE as u64;
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
    unreachable!();
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

struct KernelDevice {}

impl Device for KernelDevice {
    fn ready(&self) -> bool {
        true
    }

    fn name(&self) -> String {
        "KERNEL".to_string()
    }

    fn uuid(&self) -> Uuid {
        *device::well_known::IPL
    }
}

fn kernel_main() -> ! {
    let status_bits = arch::arch_x86_64::cpu::get_online_cpu_status_bits();
    {
        let online_cpus = status_bits.lock().iter().filter(|b| *b == true).count();

        debug!("Boot complete with {} CPUs online.", online_cpus);
    }
    verbose!(
        "Oxidized kernel v{}, starting",
        METADATA_VERSION.unwrap_or("Unknown")
    );
    verbose!("CPU Vendor: {}", get_cpu_vendor_string());
    verbose!("CPU Brand : {}", get_cpu_brand_string());

    let mut device_tree = get_mut_device_tree();
    let root_device = device_tree.register(KernelDevice{});
    debug!("Registered kernel device ({}) as {:032X}", device::well_known::IPL.as_hyphenated(), root_device);
    debug!("Enumerating device tree");
    for i in device_tree.keys().iter() {
        let dev = device_tree.get(i).expect("UNKNOWN DEVICE");
        let path = device_tree.get_device_path(dev);
        // The third URI
        debug!(
            "Found: {} at sys://device/uuid/{}, sys://device/id/{:032x}, and  sys://device/path/{}/{:032x}",
            dev.name(),
            dev.uuid().as_hyphenated(),
            i,
            path,
            i
        );
    }

    set_kernel_ready();
    // Join the APIs in their halt loop glory.
    kernel_cpu_main();
}

fn kernel_cpu_main() -> ! {
    // TODO: Enter the scheduler here.
    if !kernel_ready() {
        debug!("Waiting for BSP to mark the kernel ready.");
        while !kernel_ready() {
            core::hint::spin_loop();
        }
    }
    let cpu = cpu_apic_id();
    debug!("Entered kernel_cpu_main on CPU #{}", cpu);
    loop {
        // let ticks = get_timer_ticks();
        // debug!("Tick: {}", ticks);
        wait_for_interrupt();
    }
}

fn set_kernel_ready() {
    unsafe {
        let val = READY_SIGNAL.get_mut();
        *val = true;
    }
}

fn kernel_ready() -> bool {
    unsafe {
        let lock = READY_SIGNAL.lock();
        *lock
    }
}

static mut READY_SIGNAL: Mutex<bool> = Mutex::new(false);
