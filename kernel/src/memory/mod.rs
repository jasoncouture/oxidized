use alloc::string::String;
use bootloader_api::info::MemoryRegions;
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::{
    registers::control::Cr3,
    structures::paging::{mapper::TranslateResult, *},
    PhysAddr, VirtAddr,
};

use crate::{println, verbose};

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

    pub fn identity_map_writable_data_for_kernel(&mut self, physical_address: PhysAddr) {
        self.identity_map(
            physical_address,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE,
        );
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
        }
        let mut start_page = Page::<Size4KiB>::containing_address(start_page);
        let page_table = self.page_table.as_mut().unwrap();
        loop {
            let end_page = start_page + pages as u64;
            println!(
                "Processing page range: {:#016X} to {:#016X}",
                start_page.start_address().as_u64(),
                end_page.start_address().as_u64()
            );
            let mut start_over = false;
            for page in Page::<Size4KiB>::range_inclusive(start_page, end_page) {
                if let Ok(_) = page_table.translate_page(page) {
                    let next_start = start_page.start_address() + PAGE_SIZE as u64;
                    if start_page == page {
                        self.next_free_page = next_start;
                    }
                    println!(
                        "Page range conflicts with {}, starting at next page: {}",
                        page.start_address().as_u64(),
                        next_start.as_u64()
                    );
                    start_page = Page::containing_address(next_start);
                    start_over = true;
                    break;
                }
            }

            if (start_over) {
                continue;
            }

            for page in Page::<Size4KiB>::range_inclusive(start_page, end_page) {
                let frame = unsafe { KERNEL_FRAME_ALLOCATOR.allocate_frame() };
                if frame.is_none() {
                    return None;
                }
                let frame = frame.unwrap();
                let result =
                    unsafe { page_table.map_to(page, frame, flags, &mut KERNEL_FRAME_ALLOCATOR) };
                let result = result.expect("Failed to map virtual memory!");
                result.flush();
            }
            return Some(start_page.start_address().as_mut_ptr());
        }
    }

    pub fn identity_map(&mut self, address: PhysAddr, flags: PageTableFlags) {
        let frame = PhysFrame::<Size4KiB>::containing_address(address);
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
) -> *mut u8 {
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
        println!("Getting SMP Trampoline frame");
        let next_page = KERNEL_FRAME_ALLOCATOR.allocate_frame();
        let pointer = match next_page {
            Some(p) => p.start_address().as_u64() as *mut u8,
            None => panic!("Could not allocate ipi trampoline frame!"),
        };
        println!("Identity mapping {:p} for SMP Trampoline", pointer);
        {
            let mut memory_manager = KERNEL_MEMORY_MANAGER.lock();
            memory_manager.identity_map(
                PhysAddr::new(pointer as u64),
                PageTableFlags::NO_CACHE | PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
            );
        }
        println!("Initializing heap");
        // And then the heap.
        init_kernel_heap().expect("Failed to initialize kernel heap");

        verbose!("Heap and virtual memory initialized.");
        pointer
    }
}
