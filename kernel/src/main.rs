#![no_std]
#![no_main]
#![feature(const_mut_refs)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner::test_runner)]
#![reexport_test_harness_main = "test_main"]
include!(concat!(env!("OUT_DIR"), "/metadata_constants.rs"));
mod test_runner;
mod panic;
mod unit_tests;
mod framebuffer;

const CONFIG: bootloader_api::BootloaderConfig = {
    let mut config = bootloader_api::BootloaderConfig::new_default();
    config.kernel_stack_size = 100 * 1024; // 100 KiB
    config.mappings.aslr = true;
    config
};

bootloader_api::entry_point!(kernel_boot, config = &CONFIG);


#[allow(unreachable_code)]
fn kernel_boot(boot_info: &'static mut bootloader_api::BootInfo) -> ! {

    early_init(boot_info);
    test_hook();
    kernel_main();
    loop { 
        x86_64::instructions::interrupts::disable();
        x86_64::instructions::hlt();
    }
}

fn early_init(boot_info: &'static mut bootloader_api::BootInfo) {
    
}

fn kernel_main() {
    
}

fn test_hook() {
    #[cfg(test)]
    test_main();
}


