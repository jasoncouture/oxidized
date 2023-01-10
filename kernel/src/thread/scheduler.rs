use core::cell::OnceCell;
use lazy_static::*;

pub struct Scheduler {}

static mut SCHEDULER: OnceCell<Scheduler> = OnceCell::new();
