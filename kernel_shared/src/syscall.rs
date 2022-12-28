use core::arch::asm;

use crate::constants::*;

//#[cfg(any(target_feature = "client", target_feature = "server"))]
#[cfg(target_arch = "x86_64")]
pub extern "C" fn syscall(function: SyscallNumber, parameters: *const u8) -> *const u8 {
    let mut ret: usize = 0;
    unsafe {
        asm!("syscall",
            in("rax") function as usize,
            in("rdi") parameters as usize,
            lateout("rax") ret,
            out("rcx") _, // rcx is used to store old rip
            out("r11") _, // r11 is used to store old rflags
            options(nostack, preserves_flags)
        );
    }

    ret as *const u8
}

#[cfg(target_arch = "x86")]
pub unsafe fn syscall1(n: SyscallNumber, arg1: usize) -> usize {
    let mut ret: usize;
    unsafe {
        asm!(
            "int 0x80",
            inlateout("eax") n as usize => ret,
            in("ebx") arg1,
            options(nostack, preserves_flags)
        );
    }
    ret
}
