use x86_64::{
    structures::paging::{OffsetPageTable, PageTable, PhysFrame, Size4KiB},
    PhysAddr, VirtAddr,
};

use crate::arch::{PageFlags, VirtualMemoryManager};

pub(crate) type PlatformMemoryAddressIntegerType = u64;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PlatformVirtualMemoryManager {
    page_table: *mut PageTable,
    physical_memory_offset: PlatformMemoryAddress,
}
impl PlatformVirtualMemoryManager {
    pub fn new(page_table: *mut PageTable, physical_offset: PlatformMemoryAddress) -> Self {
        Self {
            page_table,
            physical_memory_offset: physical_offset,
        }
    }
}

impl VirtualMemoryManager for PlatformVirtualMemoryManager {
    fn map_page(
        &self,
        physical_address: PlatformMemoryAddress,
        virtual_address: PlatformMemoryAddress,
        flags: PageFlags,
    ) {
        unsafe {
            let page_table = unsafe { self.page_table.as_mut() }.unwrap();
            let offset_page_table =
                OffsetPageTable::new(page_table, self.physical_memory_offset.to_virtual_address());
            
        }
    }

    fn set_page_flags(&self, virtual_address: PlatformMemoryAddress, flags: PageFlags) {
        todo!()
    }

    fn get_page_flags(&self, virtual_address: PlatformMemoryAddress) -> Option<PageFlags> {
        todo!()
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PlatformMemoryAddress(PlatformMemoryAddressIntegerType);

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
    pub fn to_platform_value(&self) -> PlatformMemoryAddressIntegerType {
        self.0
    }

    pub fn from_page_number(page_number: usize) -> Self {
        Self((page_number << 12) as u64)
    }

    pub(crate) fn to_page_number(&self) -> usize {
        return (self.0 >> 12) as usize;
    }

    pub(crate) fn to_pointer(&self) -> *mut u8 {
        self.0 as *mut u8
    }

    pub(crate) fn to_physical_frame(&self) -> Option<PhysFrame<Size4KiB>> {
        Some(PhysFrame::from_start_address(self.to_physical_address()).unwrap())
    }
}
