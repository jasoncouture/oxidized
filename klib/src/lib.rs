#![no_std]

use core::arch::asm;


#[cfg(target_arch = "x86_64")]
#[no_mangle]
pub extern "C" fn kmemcpy(destination_address: usize, source_address: usize, count: usize) {
    unsafe {
        asm!(
            "
                cld
                rep movsb
            ",
            in("rcx") count,
            in("rdi") destination_address,
            in("rsi") source_address
        );
    }
}

#[cfg(target_arch = "x86")]
#[no_mangle]
pub extern "C" fn kmemcpy(destination_address: usize, source_address: usize, count: usize) {
    unsafe {
        asm!(
            "
                cld
                rep movsb
            ",
            in("ecx") count,
            in("edi") destination_address,
            in("esi") source_address
        );
    }
}