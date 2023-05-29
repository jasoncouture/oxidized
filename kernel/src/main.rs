#![no_std]
#![no_main]
#![feature(const_mut_refs)]
#![feature(custom_test_frameworks)]
#![feature(slice_pattern)]
#![feature(abi_x86_interrupt)]
#![feature(asm_const)]
#![feature(naked_functions)]
#![feature(pointer_byte_offsets)]
#![feature(core_intrinsics)]
#![feature(pointer_is_aligned)]
#![feature(error_in_core)]
#![feature(allocator_api)]
#![feature(slice_ptr_get)]

pub(crate) mod arch;
pub(crate) mod logging;
mod memory;
pub(crate) mod panic;
pub(crate) mod serial;

extern crate alloc;
use arch::Hal;
use klib::get_size_suffix_and_divisior;

use crate::{arch::{PageState, Platform}, memory::page_allocator::PageAllocator};

fn kmain(hal: &mut Hal) {
    info!("HAL Initialized for {}", hal.get_platform_arch());
    let memory_map = hal.get_compact_memory_map();
    info!("Boot memory map:");
    let mut memory_size_bytes = 0;
    let mut memory_reserved_bytes = 0;
    for i in memory_map.iter() {
        info!("   {:?}", i);
        let size = i.end.to_platform_value() - i.start.to_platform_value();
        memory_size_bytes = memory_size_bytes + size;
        if i.page_state == PageState::Used {
            memory_reserved_bytes = memory_reserved_bytes + size;
        }
    }
    let size_suffix_and_divisor = get_size_suffix_and_divisior(memory_size_bytes);
    info!(
        "Total system memory:    {}{}",
        memory_size_bytes / size_suffix_and_divisor.1 + 1,
        size_suffix_and_divisor.0
    );
    let size_suffix_and_divisor = get_size_suffix_and_divisior(memory_reserved_bytes);
    info!(
        "Reserved system memory: {}{}",
        memory_reserved_bytes / size_suffix_and_divisor.1 + 1, 
        size_suffix_and_divisor.0
    );

    let size_suffix_and_divisor = get_size_suffix_and_divisior(memory_size_bytes - memory_reserved_bytes);
    info!(
        "Free memory at boot:    {}{}",
        (memory_size_bytes - memory_reserved_bytes) / size_suffix_and_divisor.1+1,
        size_suffix_and_divisor.0
    );

    info!("Initializing page allocator");
    PageAllocator::initialize(hal);
    info!("Allocating a free page for funsies");
    let page_pointer = PageAllocator::allocate_size(128).unwrap();
    info!("Allocated page at address: {:p}", page_pointer);
    PageAllocator::free_size(page_pointer, 128).unwrap();
}
