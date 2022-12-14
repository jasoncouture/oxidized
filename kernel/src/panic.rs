#[allow(unused_imports)]
use core::panic::PanicInfo;

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    #[allow(unused_imports)]
    use core::arch::x86_64;
    use pal::HardwareControl;
    use kernel_vga_buffer::{println, Color, WRITER};
    use pal_x86_64::PAL_PLATFORM;

    WRITER.lock().set_background_color(Color::Red);
    WRITER.lock().set_foreground_color(Color::White);
    println!();
    println!("HALT:");
    println!("{}", info);
    PAL_PLATFORM.halt();
}
