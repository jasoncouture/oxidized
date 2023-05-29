pub mod virtual_memory;

use alloc::{boxed::Box, vec::Vec};
use bootloader_api::{config::Mapping, BootInfo};
use x86_64::{
    structures::paging::{
        PageTable, PageTableFlags,
        PhysFrame, Size4KiB,
    },
    PhysAddr, VirtAddr,
};

use crate::{arch::PageState, debug, info, kmain};

use self::virtual_memory::{PlatformMemoryAddressIntegerType, PlatformVirtualMemoryManager};

use super::{PageRange, Platform, VirtualMemoryManager, PlatformMemoryAddress};

pub(crate) type NativePageFlags = PageTableFlags;
pub const PLATFORM_VALID_PAGE_SIZES: [PlatformMemoryAddressIntegerType; 1] = [0x1000u64];

#[derive(Debug, Clone, Copy)]
pub(crate) struct PlatformImplementation {
    kernel_virtual_memory_manager: PlatformVirtualMemoryManager, //&'static PageTable,
    boot_info: &'static BootInfo,
}



impl PlatformImplementation {
    pub fn new(boot_info: &'static mut PlatformBootInfo) -> Self {
        let physical_memory_offset = PlatformMemoryAddress::from(
            boot_info
                .physical_memory_offset
                .into_option()
                .unwrap_or_default(),
        );
        info!(
            "Initializing HAL, with base address {:#014x}",
            physical_memory_offset.to_virtual_address().as_u64()
        );

        let virtual_memory_manager = PlatformVirtualMemoryManager::new(
            unsafe {
                Self::get_active_page_table_pointer(physical_memory_offset.to_virtual_address())
            },
            physical_memory_offset,
        );
        Self {
            kernel_virtual_memory_manager: virtual_memory_manager,
            boot_info: boot_info,
        }
    }

    unsafe fn get_active_page_table_pointer(physical_memory_offset: VirtAddr) -> *mut PageTable {
        use x86_64::registers::control::Cr3;

        let (level_4_table_frame, _) = Cr3::read();

        let phys = level_4_table_frame.start_address();
        let virt = physical_memory_offset + phys.as_u64();

        debug!(
            "Using page table at physical address {:#014x} (virtual: {:#014x}) ",
            phys.as_u64(),
            virt.as_u64()
        );
        let ptr: *mut PageTable = virt.as_mut_ptr();
        ptr
    }
}

impl Platform for PlatformImplementation {
    fn to_native_page_flags(flags: super::PageFlags) -> NativePageFlags {
        NativePageFlags::from_bits_truncate(flags.bits())
    }

    fn get_memory_map(&self) -> Vec<PageRange> {
        let mut vec = Vec::<PageRange>::new();
        for i in self.boot_info.memory_regions.iter() {
            let state = match i.kind {
                bootloader_api::info::MemoryRegionKind::Bootloader => PageState::Used,
                bootloader_api::info::MemoryRegionKind::UnknownBios(_) => PageState::Used,
                bootloader_api::info::MemoryRegionKind::UnknownUefi(_) => PageState::Used,
                bootloader_api::info::MemoryRegionKind::Usable => PageState::Free,
                _ => PageState::Used,
            };

            let range = PageRange::new(
                state,
                PlatformMemoryAddress::from(i.start),
                PlatformMemoryAddress::from(i.end),
            );
            vec.insert(0, range);
        }

        vec
    }

    fn get_platform_arch(&self) -> &str {
        "x86_64"
    }
}

pub type PlatformBootInfo = BootInfo;

#[allow(unreachable_code)]
fn _entry(boot_info: &'static mut BootInfo) -> ! {
    let mut hal = super::initialize_hal(boot_info);
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
    config
};

bootloader_api::entry_point!(_entry, config = &CONFIG);
