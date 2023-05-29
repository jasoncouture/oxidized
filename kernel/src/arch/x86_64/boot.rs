use bootloader_api::{config::Mapping, BootInfo};
use klib::MIB;

use crate::{arch::initialize_hal, kmain};

pub type PlatformBootInfo = BootInfo;

// Boot code for platform.

#[allow(unreachable_code)]
fn _entry(boot_info: &'static mut PlatformBootInfo) -> ! {
    let mut hal = initialize_hal(boot_info);
    kmain(&mut hal);
    panic!("Kernel main exited");
}

const KERNEL_ADDRESS_RANGE_START: u64 = 0x0000000000100000u64;
const KERNEL_ADDRESS_RANGE_END: u64 = 0x0000FFFFFFFFFFFFu64;
const CONFIG: bootloader_api::BootloaderConfig = {
    let mut config = bootloader_api::BootloaderConfig::new_default();
    config.mappings.aslr = true;
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config.mappings.dynamic_range_start = Some(KERNEL_ADDRESS_RANGE_START);
    config.mappings.dynamic_range_end = Some(KERNEL_ADDRESS_RANGE_END);
    config.mappings.ramdisk_memory = Mapping::Dynamic;
    config.kernel_stack_size = (16 * MIB) as u64;
    config
};

bootloader_api::entry_point!(_entry, config = &CONFIG);