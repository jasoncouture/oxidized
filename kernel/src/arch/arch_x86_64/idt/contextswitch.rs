use core::arch::asm;

use crate::{
    arch::{
        arch_x86_64::gdt::{get_gdt, INTERRUPT_STACK_SIZE},
        get_current_cpu,
    },
    debug,
};

#[naked]
pub unsafe extern "C" fn _context_switch() {
    asm!(
        "
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

    mov rdi, rsp
    mov rsi, rsp
    xor rax, rax

    // Hi ho, hi ho, back to the scheduler we go
    call context_switch
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
    iretq
    ",
        options(noreturn)
    );
}
#[derive(Debug, Clone, Copy)]
#[repr(C, align(8))]
pub struct PlatformContextState {
    registers: RegisterState,
    sse: Option<[u8; 512]>,
    tss: Option<[u8; INTERRUPT_STACK_SIZE]>,
}

impl PlatformContextState {
    pub fn new() -> Self {
        let gdt = get_gdt(get_current_cpu());
        let cs = gdt.get_user_code_segment().index() as u64;
        let ds = gdt.get_user_data_segment().index() as u64;
        let mut registers = RegisterState::default();
        registers.cs = cs;
        registers.ss = ds;

        Self {
            registers,
            sse: None,
            tss: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
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
    rip: u64,
    cs: u64,
    rflags: u64,
    rsp: u64,
    ss: u64,
}

#[no_mangle]
unsafe extern "C" fn context_switch(state: *mut RegisterState, state_address: usize) {
    debug!(
        "Context switch requested, context state stored at {:p} ({:016x}",
        state, state_address,
    );
}

fn save_fpu(buffer: &mut [u8; 512]) {
    unsafe {
        asm!(
            "fxsave64 [{}]", 
            in(reg) buffer as *mut _)
    }
}

fn restore_fpu(buffer: &[u8; 512]) {
    unsafe {
        asm!(
            "fxrstor64 [{}]",
            in(reg) buffer as *const _
        )
    }
}
