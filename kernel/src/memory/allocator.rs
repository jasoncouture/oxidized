use core::alloc::{GlobalAlloc, Layout};

use bitvec::prelude::*;
use bootloader_api::info::{MemoryRegionKind, MemoryRegions};

use linked_list_allocator::LockedHeap;
use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, PageSize, PageTableFlags, PhysFrame, Size4KiB,
    },
    PhysAddr, VirtAddr,
};

use crate::{debug, println};

use super::KERNEL_MEMORY_MANAGER;

#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("allocation error: {:?}", layout);
}

struct KernelAllocator(LockedHeap);

impl KernelAllocator {
    pub fn init(&mut self) {
        let mut locked_allocator = self.0.lock();
        let heap_space = Self::allocate_heap_space(KERNEL_HEAP_PAGES);
        unsafe {
            locked_allocator.init(heap_space, KERNEL_HEAP_PAGES * Size4KiB::SIZE as usize);
        }
    }

    pub const fn empty() -> KernelAllocator {
        KernelAllocator(LockedHeap::empty())
    }

    fn allocate_heap_space(pages: usize) -> *mut u8 {
        let mut locked_memory_manager = KERNEL_MEMORY_MANAGER.lock();
        locked_memory_manager
            .allocate_contigious_address_range(
                pages,
                Some(VirtAddr::new(KERNEL_HEAP_START as u64)),
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE,
            )
            .expect("Failed to allocate heap!")
    }

    fn extend_heap(&self, needed_bytes: usize) {
        let mut locked_allocator = self.0.lock();
        let current_size = locked_allocator.size();
        if current_size == 0 {
            panic!("Attempted to extend an uninitialized heap!");
        }

        let mut pages_to_allocate = (current_size / PAGE_SIZE) + 1;
        let needed_pages = ((needed_bytes * 8) / PAGE_SIZE) + 1;

        if pages_to_allocate < needed_pages {
            pages_to_allocate = needed_pages;
        }

        if Self::allocate_heap_space(pages_to_allocate) as usize == 0 {
            panic!("Ran out of memory attempting to extend to heap!");
        }

        unsafe { locked_allocator.extend(pages_to_allocate * PAGE_SIZE) };
    }
}

#[global_allocator]
static mut ALLOCATOR: KernelAllocator = KernelAllocator::empty();

unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let ret = self.0.alloc(layout);
        if ret as usize != 0 {
            return ret;
        }
        let needed_size = layout.size() + layout.align();
        self.extend_heap(needed_size);
        self.0.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        self.0.dealloc(ptr, layout)
    }
}

pub const PAGE_SIZE: usize = 4096;
pub const KERNEL_HEAP_START: usize = 0x_F000_0000_0000;
pub const KERNEL_HEAP_PAGES: usize = 1;
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

        for region in self
            .memory_map
            .unwrap()
            .iter()
            .filter(|r| r.kind != MemoryRegionKind::Usable)
            .map(|r| r.start..r.end)
            .flat_map(|r| r.step_by(PAGE_SIZE))
        {
            let page = Self::get_page(region as usize);
            if page < self.used_pages.len() {
                continue; // This memory is not addressable.
            }
            self.used_pages.set(page, true);
        }
        let mut next = 0;
        for frame in self.usable_frames() {
            if frame.start_address().as_u64() < 0x100000 {
                next += 1;
                continue;
            }
            break;
        }

        self.next = next;
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

    fn is_usable(&self, page: usize) -> bool {
        if page >= self.used_pages.len() {
            return false;
        }
        let address = (page << 12) as u64;
        for region in self.memory_map.unwrap().iter() {
            if address < region.start || address >= region.end {
                continue;
            }
            if region.kind != MemoryRegionKind::Usable {
                break;
            }

            if self.used_pages[page] {
                return false;
            }

            return true;
        }
        false
    }

    #[inline]
    fn get_page(frame: usize) -> usize {
        frame >> 12
    }
    pub fn free(self: &mut Self, frame: PhysAddr) {
        let page = Self::get_page(frame.as_u64() as usize);
        self.used_pages.set(page, false);
    }

    pub fn allocate_conventional_memory_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        let mut current_frame = PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(0));
        for frame in self
            .usable_frames()
            .filter(|f| f.start_address().as_u64() < 0x100000)
        {
            let frame_address = frame.start_address().as_u64() as usize;
            // Same as / 4096, but faster.
            let page = Self::get_page(frame_address);
            current_frame += 1;
            if !self.used_pages[page] {
                self.used_pages.set(page, true);
                println!("Allocated conventional page: {}", page);
                return Some(frame);
            }
        }

        None
    }

    pub fn force_allocate(&mut self, frame: PhysFrame) -> Option<PhysFrame> {
        let page = Self::get_page(frame.start_address().as_u64() as usize);
        if self
            .used_pages
            .get(page)
            .expect("Attempted to force allocate an address above supported address range!")
            == true
        {
            panic!("Attempted to force allocate a used page!");
        }

        self.used_pages.set(page, true);

        Some(frame)
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        loop {
            let mut current_frame = self.next;
            for frame in self.usable_frames().skip(current_frame) {
                let frame_address = frame.start_address().as_u64() as usize;
                if frame_address < 0x100000 {
                    println!("Skipping conventional memory frame {:?}, conventional memory must be explicitly allocated.", frame);
                    continue;
                }
                // Same as / 4096, but faster.
                let page = Self::get_page(frame_address);
                current_frame += 1;
                if page >= self.used_pages.len() {
                    println!(
                        "Page {} is out of bounds! Starting over at first usable frame.",
                        page
                    );
                    break;
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
    println!("Initializing heap");
    unsafe { ALLOCATOR.init() };
    debug!(
        "Kernel heap Allocated heap with {} pages ({} bytes)",
        KERNEL_HEAP_PAGES,
        KERNEL_HEAP_PAGES * Size4KiB::SIZE as usize
    );
    Ok(())
}

pub fn kmalloc(layout: Layout) -> *mut u8 {
    unsafe { ALLOCATOR.alloc_zeroed(layout) }
}

pub fn kfree(ptr: *mut u8, layout: Layout) {
    unsafe { ALLOCATOR.dealloc(ptr, layout) }
}
