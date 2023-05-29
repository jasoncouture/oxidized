use core::{
    alloc::{AllocError, Allocator, GlobalAlloc, Layout},
    ptr::NonNull,
    slice,
};

use buddyalloc::Heap;
use spin::mutex::SpinMutex;

use crate::debug;

// We will need a dynamically expandable heap...
const HEAP_SIZE: usize = 1024*1024*128; // 128mb

static mut EARLY_HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

#[global_allocator]
pub(crate) static ALLOCATOR: LockedHeap<16> =
    LockedHeap::new(unsafe { &EARLY_HEAP as *const _ as *mut u8 }, HEAP_SIZE);


#[derive(Debug)]
pub(crate) struct LockedHeap<const N: usize>(SpinMutex<Heap<N>>);

impl<const N: usize> LockedHeap<N> {
    pub const fn new(heap_address: *mut u8, size: usize) -> Self {
        unsafe {
            Self(SpinMutex::<Heap<N>>::new(Heap::new_unchecked(
                heap_address,
                size,
            )))
        }
    }
    fn heap_allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let mut heap = self.0.lock();

        let ptr = heap.allocate(layout).map_err(|_| AllocError)?;
        debug!("ALLOC: {:p} - {:?}", ptr, layout);
        // SAFETY: The pointer is guaranteed to not be NULL if the heap didn't return an error.
        Ok(unsafe { NonNull::new_unchecked(slice::from_raw_parts_mut(ptr, layout.size())) })
    }

    unsafe fn heap_deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        debug!("FREE : {:p} - {:?}", ptr, layout);
        let mut heap = self.0.lock();
        heap.deallocate(ptr.as_ptr(), layout);
    }
}

unsafe impl<const N: usize> GlobalAlloc for LockedHeap<N> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        match self.heap_allocate(layout) {
            Err(e) => panic!("{}", e),
            Ok(ptr) => ptr.as_mut_ptr(),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.heap_deallocate(NonNull::new(ptr).unwrap(), layout);
    }
}

unsafe impl<const N: usize> Allocator for LockedHeap<N> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.heap_allocate(layout)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        self.heap_deallocate(ptr, layout)
    }
}
