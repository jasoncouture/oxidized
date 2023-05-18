use crate::{println, info};

use bitflags::bitflags;
use self::x86_64::{PlatformImplementation, NativePageFlags};
pub(crate) use self::x86_64::PLATFORM_VALID_PAGE_SIZES;

#[cfg(target_arch = "x86_64")]
pub(crate) use self::x86_64::PlatformMemoryAddress;

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

pub(crate) trait Platform {
    fn to_native_page_flags(flags: PageFlags) -> NativePageFlags where Self : Sized;
}

static mut PLATFORM: Option<PlatformImplementation> = None;

pub(crate) fn initialize_hal(physical_memory_offset: PlatformMemoryAddress) {
    unsafe {
        if PLATFORM.is_some() {
            panic!("Attempted to re-initialize HAL");
        } else {
            info!(
                "Initializing HAL, with base address {:#014x}",
                physical_memory_offset.to_virtual_address().as_u64()
            );
            PLATFORM = Some(PlatformImplementation::new(physical_memory_offset));
        }
    }
}
