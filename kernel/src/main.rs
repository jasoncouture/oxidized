#![no_std]
#![no_main]
#![feature(const_mut_refs)]
#![feature(custom_test_frameworks)]
#![feature(slice_pattern)]
#![feature(abi_x86_interrupt)]
#![feature(asm_const)]
#![feature(naked_functions)]
#![feature(pointer_byte_offsets)]
#![feature(core_intrinsics)]
#![feature(pointer_is_aligned)]
#![feature(error_in_core)]

use bootloader_api::{
    config::{LoggerStatus, Mapping},
    BootInfo,
};
use core::ptr::NonNull;

use crate::arch::{initialize_hal, PlatformMemoryAddress};

pub(crate) mod arch;
pub(crate) mod logging;
pub(crate) mod panic;
pub(crate) mod serial;
mod memory;

//extern crate alloc;
const KERNEL_HEAP_RANGE_START: u64 = 0x0000800000000000u64;
const KERNEL_HEAP_RANGE_END: u64 = 0x00008EFFFFFFFFFFFu64;
const KERNEL_ADDRESS_RANGE_START: u64 = 0x00008F0000000000u64;
const KERNEL_ADDRESS_RANGE_END: u64 = 0x0000EFFFFFFFFFFFu64;
const CONFIG: bootloader_api::BootloaderConfig = {
    let mut config = bootloader_api::BootloaderConfig::new_default();
    config.mappings.aslr = true;
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config.mappings.dynamic_range_start = Some(KERNEL_ADDRESS_RANGE_START);
    config.mappings.dynamic_range_end = Some(KERNEL_ADDRESS_RANGE_END);
    config.mappings.ramdisk_memory = Mapping::Dynamic;
    config
};

bootloader_api::entry_point!(kernel_boot, config = &CONFIG);

#[allow(unreachable_code)]
fn kernel_boot(boot_info: &'static mut BootInfo) -> ! {
    info!("Booting");
    initialize_hal(PlatformMemoryAddress::from(
        boot_info
            .physical_memory_offset
            .into_option()
            .unwrap_or_default(),
    ));
    warn!("The kernel is incomplete.");
    todo!();
    unreachable!();
}
