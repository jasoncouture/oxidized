use bootloader_api::info::MemoryRegions;
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::{
    registers::control::Cr3,
    structures::paging::{OffsetPageTable, PageTable},
    PhysAddr, VirtAddr,
};

use crate::{println, verbose};

use self::allocator::{init_frame_allocator, init_kernel_heap};

pub(crate) mod allocator;

pub(crate) struct MemoryManager {
    page_table: Option<OffsetPageTable<'static>>,
    physical_offset: VirtAddr,
}

impl MemoryManager {
    pub fn init(self: &mut Self, page_table: OffsetPageTable<'static>) {
        self.page_table = Some(page_table);
        self.physical_offset = self.page_table.as_ref().unwrap().phys_offset();
    }

    pub fn translate(&self, physical_address: PhysAddr) -> VirtAddr {
        VirtAddr::new(physical_address.as_u64() + self.physical_offset.as_u64())
    }
}

lazy_static! {
    pub(crate) static ref KERNEL_MEMORY_MANAGER: Mutex<MemoryManager> = Mutex::new(MemoryManager {
        page_table: None,
        physical_offset: VirtAddr::zero()
    });
}

unsafe fn get_active_page_table(base_address: VirtAddr) -> &'static mut PageTable {
    let (level_4_table_frame, _) = Cr3::read();
    let phys = level_4_table_frame.start_address();
    let virt = base_address + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    &mut *page_table_ptr
}

pub(crate) fn initialize_virtual_memory(
    base_address: VirtAddr,
    memory_map: &'static MemoryRegions,
) {
    unsafe {
        {
            let mut_memory_manager = &mut KERNEL_MEMORY_MANAGER.lock();
            mut_memory_manager.init(OffsetPageTable::new(
                get_active_page_table(base_address),
                base_address,
            ));
        }
        // and boot up the frame allocator
        init_frame_allocator(memory_map);
        // And then the heap.
        init_kernel_heap().expect("Failed to initialize kernel heap");
        verbose!("Heap and virtual memory initialized.");
    }
}
