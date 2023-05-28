#![no_std]

use core::{arch::asm};

pub const KIB: usize = 1024usize;
pub const MIB: usize = KIB * 1024;
pub const GIB: usize = MIB * 1024;
pub const TIB: usize = GIB * 1024;
pub const EIB: usize = TIB * 1024;

pub fn get_size_suffix_and_divisior(num: u64) -> (&'static str, u64) {
    if num > EIB as u64 {
        return ("EiB", EIB as u64);
    } else if num > TIB as u64 {
        return ("TiB", TIB as u64);
    } else if num > GIB as u64 {
        return ("GiB", GIB as u64);
    } else if num > MIB as u64 {
        return ("MiB", MIB as u64);
    } else if num > KIB as u64 {
        return ("KiB", KIB as u64);
    } else {
        return ("bytes", 1);
    }
}

#[cfg(target_arch = "x86_64")]
#[no_mangle]
pub extern "C" fn kmemcpy(destination_address: usize, source_address: usize, count: usize) {
    use core::panic;

    unsafe {
        if count & 1 != 0 {
            asm!(
                "
                cld
                rep movsb
            ",
                in("rcx") count,
                in("rdi") destination_address,
                in("rsi") source_address
            );
        } else if count % 8 == 0 {
            asm!(
                "
                cld
                rep movsq
            ",
                in("rcx") count / 8,
                in("rdi") destination_address,
                in("rsi") source_address
            );
        } else if count % 4 == 0 {
            asm!(
                "
                cld
                rep movsd
            ",
                in("rcx") count / 4,
                in("rdi") destination_address,
                in("rsi") source_address
            );
        } else if count % 2 == 0 {
            asm!(
                "
                cld
                rep movsw
            ",
                in("rcx") count / 2,
                in("rdi") destination_address,
                in("rsi") source_address
            );
        } else {
            panic!("What?");
        }
    }
}

pub extern "C" fn kmemset(destination_address: usize, value: u8, count: usize) {
    unsafe {
        asm!(
            "
                rep stosb
            ",
            in("rdi") destination_address,
            in("rax") value as usize,
            in("rcx") count
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
