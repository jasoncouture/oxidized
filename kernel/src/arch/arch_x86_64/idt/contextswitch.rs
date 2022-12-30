use core::{arch::asm, mem};

use kernel_shared::memory::memcpy;

use crate::debug;

#[naked]
pub unsafe extern "C" fn _context_switch() {
    asm!("
    cli // iretq will restore this, but if we need to, we can re-enable interrupts inside the actual function.
    pushf
	push	r15
	push	r14
	push	r13
	push	r12
	push	r11
	push	r10
	push	r9
	push	r8
	push	rbp
	push	rdi
	push	rsi
	push	rdx
	push	rcx
	push	rbx
	push	rax
	mov		rax, cr3
	push 	rax
	mov		rax, cr2
	push	rax

    mov rsi, rax
    xor rax, rax

    // Hi ho, hi ho, back to the scheduler we go
    call context_switch
    cli
    pop rax
	mov cr2, rax
	pop rax
	mov cr3, rax
	pop	rax
	pop	rbx
	pop	rcx
	pop	rdx
	pop	rsi
	pop	rdi
	pop	rbp
	pop	r8
	pop	r9
	pop	r10
	pop	r11
	pop	r12
	pop	r13
	pop	r14
	pop	r15
    popf
    iretq
    ", options(noreturn));
}
#[derive(Debug, Clone, Copy)]
#[repr(C, align(8))]
pub struct PlatformContextState {
    registers: RegisterState,
    sse: [u8; 512],
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(8))]
pub struct RegisterState {
    cr2: u64,
    cr3: u64,
    rax: u64,
    rbx: u64,
    rcx: u64,
    rdx: u64,
    rsi: u64,
    rdi: u64,
    rbp: u64,
    r8: u64,
    r9: u64,
    r10: u64,
    r11: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,
    flags: u64,
    rip: u64,
    cs: u64,
    rflags: u64,
    rsp: u64,
    ss: u64,
}


static REGISTER_STATE_SIZE: usize = mem::size_of::<RegisterState>();

#[no_mangle]
unsafe extern "C" fn context_switch(state: *mut u8) {
    debug!(
        "Context switch requested, context state stored at {:p}",
        state
    );
    let registers = state as *const RegisterState;
    let mut context_state = PlatformContextState {
        registers: *registers,
        sse: [0u8; 512],
    };

    save_fpu(&mut context_state.sse);
    debug!("Saved state: {:?}", context_state);
    let register_state_pointer = &mut context_state.registers as *mut _ as *const u8;
    memcpy(state, register_state_pointer, REGISTER_STATE_SIZE);
    restore_fpu(&mut context_state.sse);
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
