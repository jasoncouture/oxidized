#[allow(unused_imports)]
use core::panic::PanicInfo;

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    use crate::println;

    println!("PANIC: {}", info);
    loop { 
        x86_64::instructions::interrupts::disable();
        x86_64::instructions::hlt();
    }
}
