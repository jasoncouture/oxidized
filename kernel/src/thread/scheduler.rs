use core::cell::OnceCell;

pub struct Scheduler {}

static mut SCHEDULER: OnceCell<Scheduler> = OnceCell::new();
