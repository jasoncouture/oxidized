use crate::arch::{PLATFORM_VALID_PAGE_SIZES, x86_64::virtual_memory::PlatformMemoryAddressIntegerType};
pub(crate) mod heap;
pub mod page_tracker;
pub mod page_allocator;

#[derive(Debug, Clone, Copy)]
pub struct MemoryRange {
    start_address: PlatformMemoryAddressIntegerType,
    end_address: PlatformMemoryAddressIntegerType
}

pub enum MemoryRangeError {
    StartAddressGreaterThanEndAddress,
    EmptyRange,
    RangeNotPageAligned
}

impl MemoryRange {
    pub fn new(start_address: PlatformMemoryAddressIntegerType, end_address: PlatformMemoryAddressIntegerType) -> Result<Self, MemoryRangeError> {
        if start_address > end_address {
            return Err(MemoryRangeError::StartAddressGreaterThanEndAddress);
        } else if start_address == end_address {
            return Err(MemoryRangeError::EmptyRange);
        } else if (end_address - start_address) % PLATFORM_VALID_PAGE_SIZES[0] != 0 {
            return Err(MemoryRangeError::RangeNotPageAligned)
        }
        Ok(Self{start_address, end_address})
    }
    pub fn size(&self) -> PlatformMemoryAddressIntegerType {
        self.end_address - self.start_address
    }
    pub fn pages(&self) -> PlatformMemoryAddressIntegerType {
        self.size() / self.page_size()
    }
    pub fn page_size(&self) -> PlatformMemoryAddressIntegerType {
        for i in (0..PLATFORM_VALID_PAGE_SIZES.len()).rev() {
            if self.size() % PLATFORM_VALID_PAGE_SIZES[i] == 0 {
                return PLATFORM_VALID_PAGE_SIZES[i];
            }
        }
        unreachable!()
    }
}