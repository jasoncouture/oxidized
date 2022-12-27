use bitvec::prelude::*;
use bootloader_api::info::{MemoryRegionKind, MemoryRegions};
use linked_list_allocator::LockedHeap;
use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, PhysFrame, Size4KiB,
    },
    PhysAddr, VirtAddr,
};

use crate::println;

use super::KERNEL_MEMORY_MANAGER;

#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

pub const PAGE_SIZE: usize = 4096;
pub const KERNEL_HEAP_START: usize = 0x_4444_4444_0000;
pub const KERNEL_HEAP_PAGES: usize = 512;
pub const KERNEL_HEAP_SIZE: usize = KERNEL_HEAP_PAGES * PAGE_SIZE;
pub const ONE_MEGABYTE: usize = 1024 * 1024;
pub const ONE_GIGABTYE: usize = ONE_MEGABYTE * 1024;
pub const ONE_TERABYTE: usize = ONE_GIGABTYE * 1024;
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
pub const MAX_SUPPORTED_MEMORY: usize = ONE_TERABYTE * 8;
#[cfg(target_arch = "x86")]
pub const MAX_SUPPORTED_MEMORY: usize = ONE_GIGABTYE * 4;
pub const MAX_SUPPORTED_PAGES: usize = MAX_SUPPORTED_MEMORY / PAGE_SIZE;
pub const PAGE_STORAGE_SIZE: usize = MAX_SUPPORTED_PAGES / 8;

pub struct BootInfoFrameAllocator {
    memory_map: Option<&'static MemoryRegions>,
    next: usize,
    used_pages: BitArray<[u8; PAGE_STORAGE_SIZE]>,
}

pub static mut KERNEL_FRAME_ALLOCATOR: BootInfoFrameAllocator = BootInfoFrameAllocator {
    memory_map: None,
    next: 0,
    used_pages: bitarr![const u8, Lsb0; 0u8; MAX_SUPPORTED_PAGES],
};

impl BootInfoFrameAllocator {
    pub fn get_memory_regions(self: &Self) -> &MemoryRegions {
        self.memory_map.unwrap()
    }
    /// Create a FrameAllocator from the passed memory map.
    ///
    /// This function is unsafe because the caller must guarantee that the passed
    /// memory map is valid. The main requirement is that all frames that are marked
    /// as `USABLE` in it are really unused.
    pub unsafe fn init(self: &mut Self, memory_map: &'static MemoryRegions) {
        self.memory_map = Some(memory_map);
    }

    /// Returns an iterator over the usable frames specified in the memory map.
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        // get usable regions from memory map
        let regions = self
            .memory_map
            .expect("Memory map was not set prior to attempting to iterate frames")
            .iter();
        let usable_regions = regions.filter(|r| r.kind == MemoryRegionKind::Usable);
        // map each region to its address range
        let addr_ranges = usable_regions.map(|r| r.start..r.end);
        // transform to an iterator of frame start addresses
        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));
        // create `PhysFrame` types from the start addresses
        frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
    #[inline]
    fn get_page(frame: usize) -> usize {
        frame >> 12
    }
    pub fn free(self: &mut Self, frame: PhysAddr) {
        let page = Self::get_page(frame.as_u64() as usize);
        self.used_pages.set(page, false);
    }
}
unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        loop {
            let mut current_frame = self.next;
            for frame in self.usable_frames().skip(current_frame) {
                let frame_address = frame.start_address().as_u64() as usize;
                if frame_address == 0 {
                    continue;
                }
                // Same as / 4096, but faster.
                let page = Self::get_page(frame_address);
                current_frame += 1;
                if page >= self.used_pages.len() {
                    continue;
                }
                if !self.used_pages[page] {
                    self.next = current_frame;
                    self.used_pages.set(page, true);
                    return Some(frame);
                }
            }
            // if we started at 0, we're out of physical memory...
            if self.next == 0 {
                println!("Failed to allocate memory page!");
                return None;
            }
            println!("Failed to find a free page, resetting start offset and trying again.");
            // otherwise, restart our scan at the first page.
            self.next = 0;
        }
    }
}
pub fn init_frame_allocator(memory_map: &'static MemoryRegions) {
    unsafe {
        KERNEL_FRAME_ALLOCATOR.init(memory_map);
    }
}
pub fn init_kernel_heap() -> Result<(), MapToError<Size4KiB>> {
    unsafe {
        let mut locked_memory_manager = KERNEL_MEMORY_MANAGER.lock();
        let mapper = locked_memory_manager.page_table.as_mut().unwrap();
        let frame_allocator = &mut KERNEL_FRAME_ALLOCATOR;
        let page_range = {
            let heap_start = VirtAddr::new(KERNEL_HEAP_START as u64);
            let heap_end = heap_start + KERNEL_HEAP_SIZE - 1u64;
            let heap_start_page = Page::containing_address(heap_start);
            let heap_end_page = Page::containing_address(heap_end);
            Page::range_inclusive(heap_start_page, heap_end_page)
        };
        for page in page_range {
            let frame = frame_allocator
                .allocate_frame()
                .ok_or(MapToError::FrameAllocationFailed)?;
            let flags =
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE;
            mapper.map_to(page, frame, flags, frame_allocator)?.flush();
        }

        // We've mapped the kernel heap to physical ranges, now we just need to tell the allocator about it.
        let virt_addr_start = VirtAddr::new(KERNEL_HEAP_START as u64);
        ALLOCATOR
            .lock()
            .init(virt_addr_start.as_mut_ptr(), KERNEL_HEAP_SIZE);
    }

    Ok(())
}
