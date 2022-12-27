pub(crate) mod idt;
pub(crate) mod gdt;
mod syscall;
use pic8259::ChainedPics;

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

pub fn init_hardware() {
    gdt::init();
    idt::init();
    unsafe { PICS.lock().initialize() }; // initialize the pics, and immediately disable them.
    syscall::init();
}

pub fn breakpoint_hardware() {
    x86_64::instructions::interrupts::int3();
}