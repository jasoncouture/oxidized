#[cfg(target_arch = "x86_64")]
pub(crate) mod arch_x86_64;
use alloc::string::String;
#[cfg(target_arch = "x86_64")]
use arch_x86_64::*;
use bootloader_api::BootInfo;

use self::arch_x86_64::idt::get_timer_ticks_hardware;

#[inline]
pub fn init(boot_info: &BootInfo) {
    init_hardware(boot_info);
}

#[inline]
pub fn breakpoint() {
    breakpoint_hardware();
}

#[inline]
pub fn processor_vendor() -> String {
    get_cpu_vendor_string()
}

#[inline]
pub fn processor_brand() -> String {
    get_cpu_brand_string()
}

#[inline]
pub fn enable_interrupts() {
    enable_interrupts_hardware();
}

#[inline]
pub fn wait_for_interrupt() {
    wait_for_interrupt_hardware();
}

#[inline]
pub fn get_timer_ticks() -> usize {
    get_timer_ticks_hardware()
}