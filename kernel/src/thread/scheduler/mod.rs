use lazy_static::*;
pub(crate) mod process;

pub struct Scheduler {

}

lazy_static!{
    pub static ref SCHEDULER: Scheduler = Scheduler{};
}