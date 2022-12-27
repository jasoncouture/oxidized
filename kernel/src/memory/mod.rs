use bootloader_api::info::{MemoryRegionKind, MemoryRegions};
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::{
    registers::control::Cr3,
    structures::paging::{
        page_table::FrameError, FrameAllocator, OffsetPageTable, PageTable, PhysFrame, Size4KiB,
    },
    PhysAddr, VirtAddr,
};

use crate::println;

use self::allocator::{
    init_frame_allocator, init_kernel_heap, BootInfoFrameAllocator, KERNEL_FRAME_ALLOCATOR,
};

pub(crate) mod allocator;

pub(crate) struct MemoryManager {
    page_table: Option<OffsetPageTable<'static>>,
}
use core::mem;

pub const WORD_SIZE: usize = mem::size_of::<usize>();

impl MemoryManager {
    pub fn init(self: &mut Self, page_table: OffsetPageTable<'static>) {
        self.page_table = Some(page_table);
    }
}

lazy_static! {
    static ref KERNEL_MEMORY_MANAGER: Mutex<MemoryManager> =
        Mutex::new(MemoryManager { page_table: None });
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
            println!("Initializing memory manager");
            mut_memory_manager.init(OffsetPageTable::new(
                get_active_page_table(base_address),
                base_address,
            ));
        }
        // and boot up the frame allocator
        println!("Setting up frame allocator");
        init_frame_allocator(memory_map);
        // And then the heap.
        println!("Setting up heap");
        init_kernel_heap().expect("Failed to initialize kernel heap");
    }
}


/// Memcpy
///
/// Copy N bytes of memory from one location to another.
///
/// This faster implementation works by copying bytes not one-by-one, but in
/// groups of 8 bytes (or 4 bytes in the case of 32-bit architectures).
#[no_mangle]
pub unsafe extern fn memcpy(dest: *mut u8, src: *const u8,
                            n: usize) -> *mut u8 {

    let n_usize: usize = n/WORD_SIZE; // Number of word sized groups
    let mut i: usize = 0;

    // Copy `WORD_SIZE` bytes at a time
    let n_fast = n_usize*WORD_SIZE;
    while i < n_fast {
        *((dest as usize + i) as *mut usize) =
            *((src as usize + i) as *const usize);
        i += WORD_SIZE;
    }

    // Copy 1 byte at a time
    while i < n {
        *((dest as usize + i) as *mut u8) = *((src as usize + i) as *const u8);
        i += 1;
    }

    dest
}