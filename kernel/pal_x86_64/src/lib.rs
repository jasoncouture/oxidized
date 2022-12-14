#![no_std]

use pal::HardwareControl;

pub mod serial;
pub mod qemu_test_helpers;

pub struct Platform { }

impl HardwareControl for Platform {
    fn halt(&self) -> ! {
        loop { 
            x86_64::instructions::hlt();
        }
    }

    fn init(&self) {
        // TODO
    }
}


pub static PAL_PLATFORM: Platform = Platform{};