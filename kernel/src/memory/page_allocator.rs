use super::page_tracker::{PageTracker, PageTrackerError};
use crate::arch::{Hal, PageState, Platform, PlatformMemoryAddress, PLATFORM_VALID_PAGE_SIZES};
use lazy_static::lazy_static;
use spin::mutex::SpinMutex;
use x86_64::structures::paging::{FrameAllocator, PhysFrame, Size4KiB};
lazy_static! {
    static ref PAGE_TRACKER: SpinMutex<PageTracker> = SpinMutex::new(PageTracker::new(0));
}

#[repr(transparent)]
pub(crate) struct PageAllocator {}

impl PageAllocator {
    #[inline(always)]
    pub fn instance() -> Self {
        Self {}
    }
    pub(crate) fn initialize(hal: &mut Hal) {
        let memory_map = hal.get_compact_memory_map();
        let mut tracker = PAGE_TRACKER.lock();
        let max_address = memory_map
            .iter()
            .max_by(|left, right| left.end.cmp(&right.end))
            .unwrap();
        let max_address = max_address.end;

        match tracker.resize(max_address.to_page_number() + 1) {
            Ok(_) => (),
            Err(e) => panic!("Unable to allocate page tracker: {:?}", e),
        }

        for page_range in memory_map.iter() {
            let start_page = page_range.start.to_page_number();
            let end_page = page_range.end.to_page_number();
            let count = end_page - start_page;

            match page_range.page_state {
                PageState::Free => tracker.free_range(start_page, count).unwrap(),
                PageState::Used => tracker.reserve_range(start_page, count).unwrap(),
            }
        }
        // Block the 0 page from being allocated.
        tracker.reserve(0).unwrap();
    }

    pub(crate) fn allocate_size(size: usize) -> Result<*mut u8, PageTrackerError> {
        let start_page = 1;
        let needs_extra_page = match size % PLATFORM_VALID_PAGE_SIZES[0] as usize {
            0 => 0,
            _ => 1,
        };
        let count = size / PLATFORM_VALID_PAGE_SIZES[0] as usize;
        let count = count + needs_extra_page;
        let mut tracker = PAGE_TRACKER.lock();

        let result = tracker.find_free_range(start_page, count)?;
        tracker.reserve_range(result, count)?;
        let platform_address = PlatformMemoryAddress::from_page_number(result);

        Ok(platform_address.to_pointer())
    }

    pub(crate) fn force_allocate_pages(
        start_address: PlatformMemoryAddress,
        page_count: usize,
    ) -> Result<(), PageTrackerError> {
        let mut tracker = PAGE_TRACKER.lock();
        tracker.reserve_range(start_address.to_page_number(), page_count)?;
        Ok(())
    }

    pub(crate) fn free_size(address: *mut u8, size: usize) -> Result<(), PageTrackerError> {
        let address = PlatformMemoryAddress::from(address as u64);
        let needs_extra_page = match size % PLATFORM_VALID_PAGE_SIZES[0] as usize {
            0 => 0,
            _ => 1,
        };
        let count = size / PLATFORM_VALID_PAGE_SIZES[0] as usize;
        let count = count + needs_extra_page;
        let mut tracker = PAGE_TRACKER.lock();

        tracker.free_range(address.to_page_number(), count)
    }

    pub(crate) fn allocate_range(
        start_address: PlatformMemoryAddress,
        page_count: usize,
    ) -> Result<PlatformMemoryAddress, PageTrackerError> {
        let start_page = start_address.to_page_number();
        let mut tracker = PAGE_TRACKER.lock();
        let result = tracker.find_free_range(start_page, page_count)?;

        tracker.reserve_range(result, page_count)?;

        Ok(PlatformMemoryAddress::from_page_number(result))
    }

    pub(crate) fn free_range(
        start_address: PlatformMemoryAddress,
        page_count: usize,
    ) -> Result<(), PageTrackerError> {
        let mut tracker = PAGE_TRACKER.lock();
        tracker.free_range(start_address.to_page_number(), page_count)
    }
}

unsafe impl FrameAllocator<Size4KiB> for PageAllocator {
    fn allocate_frame(&mut self) -> Option<x86_64::structures::paging::PhysFrame<Size4KiB>> {
        let result = PageAllocator::allocate_range(PlatformMemoryAddress::from(0), 1);
        match result {
            Err(_) => None,
            Ok(address) => address.to_physical_frame(),
        }
    }
}
