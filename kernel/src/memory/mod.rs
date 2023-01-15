use bootloader_api::info::MemoryRegions;
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::{
    instructions::tlb, registers::control::Cr3, structures::paging::*, PhysAddr, VirtAddr,
};

use crate::{debug, println, verbose};

use self::allocator::{init_frame_allocator, init_kernel_heap, KERNEL_FRAME_ALLOCATOR, PAGE_SIZE};

pub(crate) mod allocator;

pub(crate) struct MemoryManager {
    page_table: Option<OffsetPageTable<'static>>,
    physical_offset: VirtAddr,
    next_free_page: VirtAddr,
}

impl MemoryManager {
    pub fn init(self: &mut Self, page_table: OffsetPageTable<'static>) {
        self.page_table = Some(page_table);
        self.physical_offset = self.page_table.as_ref().unwrap().phys_offset();
    }

    // pub fn map_page(&mut self, physical_address: PhysAddr) {

    // }

    pub fn allocate_contigious_address_range(
        &mut self,
        pages: usize,
        earliest_address: Option<VirtAddr>,
        flags: PageTableFlags,
    ) -> Option<*mut u8> {
        let mut start_page = VirtAddr::new(self.next_free_page.as_u64());
        if start_page
            < earliest_address
                .unwrap_or(start_page)
                .align_down(PAGE_SIZE as u64)
        {
            start_page = earliest_address.unwrap().align_down(PAGE_SIZE as u64);
            self.next_free_page = start_page;
        }
        let mut start_page = Page::<Size4KiB>::containing_address(start_page);
        let page_table = self.page_table.as_mut().unwrap();
        let mut index: usize = 0;
        while index < pages {
            let current_page = start_page + index as u64;
            if current_page.start_address()
                < earliest_address
                    .unwrap_or(start_page.start_address())
                    .align_down(PAGE_SIZE as u64)
            {
                start_page = current_page + 1;
                index = 0;
            } else if let Ok(_) = page_table.translate_page(current_page) {
                start_page = current_page + 1;
                index = 0;
            } else {
                index += 1;
            }
        }

        self.next_free_page = (start_page + index as u64).start_address();
        for i in 0..index {
            let frame = unsafe { KERNEL_FRAME_ALLOCATOR.allocate_frame()? };
            let flush = unsafe {
                page_table.map_to(
                    start_page + i as u64,
                    frame,
                    flags,
                    &mut KERNEL_FRAME_ALLOCATOR,
                )
            }
            .expect("Failed to map virtual memory");
            if pages == 1 {
                flush.flush();
            } else {
                flush.ignore();
            }
        }

        if pages > 1 {
            tlb::flush_all();
        }

        return Some(start_page.start_address().as_mut_ptr());
    }

    pub fn identity_map(&mut self, frame: PhysFrame<Size4KiB>, flags: PageTableFlags) {
        unsafe {
            self.page_table
                .as_mut()
                .unwrap()
                .identity_map(frame, flags, &mut KERNEL_FRAME_ALLOCATOR)
                .expect("Unable to identity map memory!")
                .flush();
        }
    }

    pub fn translate(&self, physical_address: PhysAddr) -> VirtAddr {
        VirtAddr::new(physical_address.as_u64() + self.physical_offset.as_u64())
    }
}

lazy_static! {
    pub(crate) static ref KERNEL_MEMORY_MANAGER: Mutex<MemoryManager> = Mutex::new(MemoryManager {
        page_table: None,
        physical_offset: VirtAddr::zero(),
        next_free_page: VirtAddr::new(0x100000).align_down(PAGE_SIZE as u64)
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
    println!("Initializing memory manager");
    unsafe {
        {
            let mut_memory_manager = &mut KERNEL_MEMORY_MANAGER.lock();
            mut_memory_manager.init(OffsetPageTable::new(
                get_active_page_table(base_address),
                base_address,
            ));
        }
        println!("Initializing frame allocator");
        // and boot up the frame allocator
        init_frame_allocator(memory_map);
        // And then the heap.
        init_kernel_heap().expect("Failed to initialize kernel heap");
        verbose!("Heap and virtual memory initialized.");
    }
}
