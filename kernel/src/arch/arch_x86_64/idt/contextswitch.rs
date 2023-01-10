use core::arch::asm;
use x86_64::structures::idt::InterruptStackFrame;

use crate::{arch::arch_x86_64::cpu, debug};

#[no_mangle]
pub(crate) extern "x86-interrupt" fn context_switch(state: InterruptStackFrame) {
    debug!("Context switch interrupt called on CPU: {}", cpu::current());
}

#[no_mangle]
unsafe extern "C" fn fork(state: *mut u8) {
    // TODO
}

fn save_fpu(buffer: &mut [u8; 512]) {
    unsafe {
        asm!(
            "fxsave [{}]", 
            in(reg) buffer as *mut _)
    }
}

fn restore_fpu(buffer: &[u8; 512]) {
    unsafe {
        asm!(
            "fxrstor [{}]",
            in(reg) buffer as *const _
        )
    }
}
