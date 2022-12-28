use alloc::{boxed::Box, vec::Vec};
use x86_64::structures::{tss::TaskStateSegment, paging::PageTable};

pub mod scheduler;
pub(crate) mod context;

pub struct Context {
    // TODO
}

pub struct Handle {
    // TODO
}
pub struct Thread {
    group_id: usize,
    process_id: usize,
    thread_id: usize,
    task_state: TaskStateSegment,
    stack: Box<[u8]>,
    offset_page_table: Box<PageTable>,
    context: Context,
    handles: Vec<Handle> 
}