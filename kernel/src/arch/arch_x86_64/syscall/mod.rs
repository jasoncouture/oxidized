use core::arch::asm;

use x86::msr;

use super::gdt::BOOT_GDT;

pub fn init() {
    // IA32_STAR[31:0] are reserved.
    // The base selector of the two consecutive segments for kernel code and the immediately
    // suceeding stack (data).
    let syscall_cs_ss_base = BOOT_GDT.get_kernel_code_segment().0;
    // The base selector of the three consecutive segments (of which two are used) for user code
    // and user data. It points to a 32-bit code segment, which must be followed by a data segment
    // (stack), and a 64-bit code segment.
    let sysret_cs_ss_base = BOOT_GDT.get_user_code_segment().0 | 3;
    let star_high = u32::from(syscall_cs_ss_base) | (u32::from(sysret_cs_ss_base) << 16);
    unsafe {
        msr::wrmsr(msr::IA32_STAR, u64::from(star_high) << 32);
        msr::wrmsr(msr::IA32_LSTAR, syscall_instruction as u64);
        msr::wrmsr(msr::IA32_FMASK, 0x0300); // Clear trap flag and interrupt enable

        let efer = msr::rdmsr(msr::IA32_EFER);
        msr::wrmsr(msr::IA32_EFER, efer | 1);
    }
}

pub unsafe extern "x86-interrupt" fn syscall_instruction() {
    asm!("sysretq", options(noreturn));
    unreachable!();
}
