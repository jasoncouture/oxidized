#[allow(unused_imports)]
use core::panic::PanicInfo;

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    use pal_x86_64::*;
    use pal::*;
    println!();
    println!("HALT:");
    println!("{}", info);
    PAL_PLATFORM.halt();
}
