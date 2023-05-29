pub(crate) use self::x86_64::PLATFORM_VALID_PAGE_SIZES;
use self::x86_64::{NativePageFlags, PlatformBootInfo, PlatformImplementation, virtual_memory::PlatformVirtualMemoryManager};
use alloc::vec::Vec;
use bitflags::bitflags;

#[cfg(target_arch = "x86_64")]
pub(crate) use self::x86_64::virtual_memory::PlatformMemoryAddress;
pub(crate) type Hal = PlatformImplementation;
pub(crate) type MemoryManager = PlatformVirtualMemoryManager;

#[cfg(target_arch = "x86_64")]
pub(crate) mod x86_64;

bitflags! {
    /// Possible flags for a page table entry.
    pub struct PageFlags: u64 {
        /// Specifies whether the mapped frame or page table is loaded in memory.
        const PRESENT =         1;
        /// Controls whether writes to the mapped frames are allowed.
        ///
        /// If this bit is unset in a level 1 page table entry, the mapped frame is read-only.
        /// If this bit is unset in a higher level page table entry the complete range of mapped
        /// pages is read-only.
        const WRITABLE =        1 << 1;
        /// Controls whether accesses from userspace (i.e. ring 3) are permitted.
        const USER_ACCESSIBLE = 1 << 2;
        /// If this bit is set, a “write-through” policy is used for the cache, else a “write-back”
        /// policy is used.
        const WRITE_THROUGH =   1 << 3;
        /// Disables caching for the pointed entry is cacheable.
        const NO_CACHE =        1 << 4;
        /// Set by the CPU when the mapped frame or page table is accessed.
        const ACCESSED =        1 << 5;
        /// Set by the CPU on a write to the mapped frame.
        const DIRTY =           1 << 6;
        /// Specifies that the entry maps a huge frame instead of a page table. Only allowed in
        /// P2 or P3 tables.
        const HUGE_PAGE =       1 << 7;
        /// Indicates that the mapping is present in all address spaces, so it isn't flushed from
        /// the TLB on an address space switch.
        const GLOBAL =          1 << 8;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PageState {
    Used,
    Free,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PageRange {
    pub page_state: PageState,
    pub start: PlatformMemoryAddress,
    pub end: PlatformMemoryAddress,
}

impl PageRange {
    fn new(state: PageState, start: PlatformMemoryAddress, end: PlatformMemoryAddress) -> Self {
        Self {
            page_state: state,
            start,
            end,
        }
    }
}



pub(crate) trait VirtualMemoryManager {
    fn map_page(&mut self, physical_address: PlatformMemoryAddress, virtual_address: PlatformMemoryAddress, flags: PageFlags);
    fn set_page_flags(&mut self, virtual_address: PlatformMemoryAddress, flags: PageFlags);
    fn get_page_flags(&mut self, virtual_address: PlatformMemoryAddress) -> Option<PageFlags>;
    fn flush_all(&self);
    fn flush(&self, virtual_address: PlatformMemoryAddress);

}

pub(crate) trait Platform {
    fn to_native_page_flags(flags: PageFlags) -> NativePageFlags;
    fn get_memory_map(&self) -> Vec<PageRange>;
    fn get_compact_memory_map(&self) -> Vec<PageRange> {
        let mut compacted = Vec::new();
        let mut raw: Vec<PageRange> = self.get_memory_map();
        raw.sort_unstable();
        for i in raw.iter() {
            if compacted.len() == 0 {
                compacted.push(*i);
                continue;
            }
            let mut last = *compacted.last().unwrap();
            if last.page_state == i.page_state && last.end == i.start {
                last.end = i.end;
                compacted.pop();
                compacted.push(last);
            } else if last.page_state == i.page_state && last.start == i.end {
                last.start = i.start;
                compacted.pop();
                compacted.push(last);
            } else {
                compacted.push(*i);
            }
        }
        compacted
    }
    fn get_platform_arch(&self) -> &str;
    fn get_kernel_memory_manager(&self) -> MemoryManager;
    fn new_memory_manager(&self) -> MemoryManager;
}

pub(crate) fn initialize_hal(boot_info: &'static mut PlatformBootInfo) -> PlatformImplementation {
    PlatformImplementation::new(boot_info)
}
