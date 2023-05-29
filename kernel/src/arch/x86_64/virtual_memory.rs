use core::{intrinsics::offset, ops::IndexMut};

use klib::kmemset;
use x86_64::{
    instructions::tlb,
    structures::paging::{
        page_table::{self, PageTableEntry},
        Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, PageTableIndex, PhysFrame,
        Size4KiB,
    },
    PhysAddr, VirtAddr,
};

use crate::{
    arch::{PageFlags, VirtualMemoryManager},
    memory::{heap::ALLOCATOR, page_allocator::PageAllocator},
};

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
    fn get_page_entry(
        &mut self,
        virtual_address: PlatformMemoryAddress,
        create: bool,
    ) -> Option<&mut PageTableEntry> {
        let address = virtual_address.to_platform_value() as usize;
        let level_1 = (address >> 12) & 0x1ff;
        let level_2 = (level_1 >> 9) & 0x1ff;
        let level_3 = (level_2 >> 9) & 0x1ff;
        let level_4 = (level_3 >> 9) & 0x1ff;
        let mut base_flags = PageTableFlags::PRESENT;
        base_flags.insert(PageTableFlags::WRITABLE);

        let page_table = unsafe { self.page_table.as_mut().unwrap() };
        let page_table = self.get_next_page_table(page_table, level_4, create, base_flags)?;
        let page_table = self.get_next_page_table(page_table, level_3, create, base_flags)?;
        let page_table = self.get_next_page_table(page_table, level_2, create, base_flags)?;
        let page_entry = page_table.index_mut(level_1);
        Some(page_entry)
    }

    fn get_next_page_table(
        &self,
        page_table: &mut PageTable,
        index: usize,
        create: bool,
        flags: PageTableFlags,
    ) -> Option<&mut PageTable> {
        let entry = page_table.index_mut(index);
        if entry.is_unused() && create {
            let addr = PageAllocator::allocate_range(PlatformMemoryAddress::from_page_number(0), 1)
                .unwrap();
            // Zero out our new page.
            kmemset(
                addr.to_platform_value() as usize
                    + self.physical_memory_offset.to_platform_value() as usize,
                0,
                4096,
            );
            entry.set_addr(addr.to_physical_address(), flags);
        } else if entry.is_unused() {
            return None;
        }

        let address = entry.addr() + self.physical_memory_offset.to_platform_value();
        let page_table_pointer = address.as_u64() as *mut PageTable;
        Some(unsafe { page_table_pointer.as_mut().unwrap() })
    }
}

impl VirtualMemoryManager for PlatformVirtualMemoryManager {
    fn map_page(
        &mut self,
        physical_address: PlatformMemoryAddress,
        virtual_address: PlatformMemoryAddress,
        flags: PageFlags,
    ) {
        let page_entry = match self.get_page_entry(virtual_address, true) {
            Some(entry) => entry,
            None => panic!("Could not map address {:p}!", virtual_address.to_pointer()),
        };

        page_entry.set_addr(
            physical_address.to_physical_address(),
            PageTableFlags::from_bits_truncate(flags.bits()),
        );
    }

    fn set_page_flags(&mut self, virtual_address: PlatformMemoryAddress, flags: PageFlags) {
        match self.get_page_entry(virtual_address, true) {
            Some(entry) => entry.set_flags(PageTableFlags::from_bits_truncate(flags.bits())),
            None => panic!("Could not map address {:p}!", virtual_address.to_pointer()),
        };
    }

    fn get_page_flags(&mut self, virtual_address: PlatformMemoryAddress) -> Option<PageFlags> {
        let entry = self.get_page_entry(virtual_address, false)?;
        Some(PageFlags::from_bits_truncate(entry.flags().bits()))
    }

    fn flush_all(&self) {
        tlb::flush_all();
    }

    fn flush(&self, virtual_address: PlatformMemoryAddress) {
        tlb::flush(virtual_address.to_virtual_address().align_down(4096 as u64));
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
