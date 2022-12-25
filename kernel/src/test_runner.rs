#[cfg(test)]
pub fn test_runner(tests: &[&dyn Fn()]) {
    //use pal_x86_64::println;

    // println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
    // println!("{} tests completed successfully", tests.len());
    test_exit(QemuExitCode::Success);
}

#[allow(dead_code)]
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x1, // Real exit code: 3
    Failed = 0x2, // Real exit code: 4
}
#[cfg(target_arch="x86_64")]
#[cfg(test)]
fn test_exit(exit_code: QemuExitCode) {
    // use pal_x86_64::qemu_test_helpers::exit_qemu;
    // exit_qemu(exit_code as u32);
}

// our panic handler in test mode
#[cfg(test)]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    // use pal::HardwareControl;
    // use pal_x86_64::{println, PAL_PLATFORM};

    // println!("[failed]\n");
    // println!("Error: {}\n", info);
    test_exit(QemuExitCode::Failed);
    loop { 
        x86_64::instructions::interrupts::disable();
        x86_64::instructions::hlt();
    }
}