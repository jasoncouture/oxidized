pub(crate) mod arch_x86_64;
#[cfg(target_arch = "x86_64")]
use arch_x86_64::*;

pub fn init() {
    init_hardware();
}

pub fn breakpoint() {
    breakpoint_hardware();
}

