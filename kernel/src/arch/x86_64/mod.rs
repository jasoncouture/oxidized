use x86_64::{PhysAddr, VirtAddr, structures::paging::{PageTable, page_table::PageTableEntry, PageTableFlags}};

use crate::{println, debug};

use super::Platform;
pub(crate) type PlatformMemoryAddressIntegerType = u64;
pub(crate) type NativePageFlags = PageTableFlags;
pub const PLATFORM_VALID_PAGE_SIZES: [PlatformMemoryAddressIntegerType; 1] = [0x1000u64];

#[derive(Debug, Clone, Copy)]
pub(crate) struct PlatformImplementation {
    main_page_table: &'static PageTable
}
pub(crate) struct PlatformMemoryAddress(PlatformMemoryAddressIntegerType);


impl PlatformMemoryAddress {
    pub fn from(address: PlatformMemoryAddressIntegerType) -> Self {
        Self(address)
    }
    pub fn to_physical_address(&self) -> PhysAddr {
        PhysAddr::new(self.0)
    }

    pub fn to_virtual_address(&self) -> VirtAddr {
        VirtAddr::new(self.0)
    }
}

impl PlatformImplementation {
    pub fn new(physical_memory_offset: PlatformMemoryAddress) -> Self {
        Self { main_page_table: unsafe { Self::get_active_page_table(physical_memory_offset.to_virtual_address()) } }
    }

    unsafe fn get_active_page_table(physical_memory_offset: VirtAddr) -> &'static PageTable {
        use x86_64::registers::control::Cr3;

        let (level_4_table_frame, _) = Cr3::read();
    
        let phys = level_4_table_frame.start_address();
        let virt = physical_memory_offset + phys.as_u64();

        debug!("Using page table at physical address {:#014x} (virtual: {:#014x}) ", phys.as_u64(), virt.as_u64());
        let page_table_ptr: *mut PageTable = virt.as_mut_ptr();
        let ret = &mut *page_table_ptr;
        ret // unsafe
    }
}

impl Platform for PlatformImplementation {
    fn to_native_page_flags(flags: super::PageFlags) -> NativePageFlags {
        NativePageFlags::from_bits_truncate(flags.bits())
    }
}