pub mod virtual_memory;
pub mod boot;

use alloc::vec::Vec;
use klib::kmemset;
use x86_64::{
    instructions::tlb,
    structures::paging::{
        page_table::{PageTableEntry, PageTableLevel},
        PageTable, PageTableFlags,
    }, VirtAddr,
};

use crate::{arch::PageState, debug, info, memory::page_allocator::PageAllocator};

use self::{virtual_memory::{PlatformMemoryAddressIntegerType, PlatformVirtualMemoryManager}, boot::PlatformBootInfo};

use super::{PageRange, Platform, PlatformMemoryAddress, MemoryManager};

pub(crate) type NativePageFlags = PageTableFlags;
pub const PLATFORM_VALID_PAGE_SIZES: [PlatformMemoryAddressIntegerType; 1] = [0x1000u64];

#[derive(Debug, Clone, Copy)]
pub(crate) struct PlatformImplementation {
    kernel_virtual_memory_manager: PlatformVirtualMemoryManager,
    boot_info: &'static PlatformBootInfo,
    kernel_page_table: *mut PageTable
}

impl PlatformImplementation {
    fn setup_page_table_entry(
        page_table_entry: &mut PageTableEntry,
        level: PageTableLevel,
        physical_memory_offset: VirtAddr,
    ) {
        if page_table_entry.is_unused() {
            return;
        }

        if page_table_entry.flags().contains(PageTableFlags::HUGE_PAGE)
            && level != PageTableLevel::One
        {
            Self::set_kernel_space_flags(page_table_entry, level);
            return;
        }

        if level == PageTableLevel::One {
            Self::set_kernel_space_flags(page_table_entry, level);
        } else {
            let next = 
                (page_table_entry.addr().as_u64() + physical_memory_offset.as_u64())
                    as *mut PageTable;
            let next = unsafe { next.as_mut().unwrap() };
            Self::walk_page_table(
                next,
                level.next_lower_level().unwrap(),
                physical_memory_offset,
            );
        }
    }

    fn set_kernel_space_flags(page_table_entry: &mut PageTableEntry, level: PageTableLevel) {
        let flags = PageTableFlags::from_bits(
            (page_table_entry.flags().bits() | PageTableFlags::GLOBAL.bits())
                & PageTableFlags::USER_ACCESSIBLE.complement().bits(),
        )
        .unwrap();
        page_table_entry.set_flags(flags);
    }

    fn walk_page_table(
        page_table: &mut PageTable,
        level: PageTableLevel,
        physical_memory_offset: VirtAddr,
    ) {
        for page_table_entry in page_table.iter_mut() {
            Self::setup_page_table_entry(page_table_entry, level, physical_memory_offset);
        }
    }
    pub fn new(boot_info: &'static mut PlatformBootInfo) -> Self {
        let physical_memory_offset = PlatformMemoryAddress::from(
            boot_info
                .physical_memory_offset
                .into_option()
                .unwrap_or_default(),
        );
        let kernel_page_table = unsafe {
            Self::get_active_page_table_pointer(physical_memory_offset.to_virtual_address())
        };
        let virtual_memory_manager = PlatformVirtualMemoryManager::new(
            kernel_page_table,
            physical_memory_offset,
        );

        Self::walk_page_table(
            unsafe { kernel_page_table.as_mut().unwrap() },
            PageTableLevel::Four,
            physical_memory_offset.to_virtual_address(),
        );

        debug!("Flushing TLB");
        tlb::flush_all();

        Self {
            kernel_virtual_memory_manager: virtual_memory_manager,
            boot_info: boot_info,
            kernel_page_table
        }
    }

    #[inline(always)]
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

    fn get_physical_memory_virtual_address(&self) -> PlatformMemoryAddress {
        PlatformMemoryAddress::from(self.boot_info.physical_memory_offset.into_option().unwrap())
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



    fn get_kernel_memory_manager(&self) -> super::MemoryManager {
        MemoryManager::new(self.kernel_page_table, self.get_physical_memory_virtual_address())
    }

    fn new_memory_manager(&self) -> super::MemoryManager {
        let frame = PageAllocator::allocate_size(4096).unwrap();
        kmemset(frame as usize, 0, 4096);
        MemoryManager::new(frame as *mut PageTable, self.get_physical_memory_virtual_address())
    }
}


