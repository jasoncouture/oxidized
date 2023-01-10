; trampoline for bringing up APs

ORG 0x0000
SECTION .text
USE16

trampoline:
    jmp short startup_ap
    times 8 - ($ - trampoline) nop
    .ready: dq 0
    .cpu_id: dq 0
    .page_table: dq 0
    .stack_start: dq 0
    .stack_end: dq 0
    .code: dq 0
    .booting: dq 0
ALIGN 16
ALIGN 4
startup_ap:
    cld
    cli
    ; zero our code segment, the assembler thinks we're in CS 0, so make it so.
    xor ax, ax
    mov cs, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov ax, 1
    ; Load IDT
    lidt [idt]
    lgdt [gdtr]
    mov eax, 1
    mov [trampoline.booting], eax

    ; compute and setup the early stack, we'll need it to preserve EDX through wrmsr
    mov ax, early_stack.end - 256
    mov sp, ax
    mov eax, 3
    mov [trampoline.booting], eax
    
    ; cr3 holds pointer to PML4
    mov eax, [trampoline.page_table]
    mov cr3, eax
    mov eax, 5
    mov [trampoline.booting], eax
    ; enable FPU
    mov eax, cr0
    ; define the bits in CR0 we want to clear
    ; cache disable (30), Not-write through (29) Task switched (3), x87 emulation (2)
    and ebx, 1 << 30 | 1 << 29 | 1 << 3 | 1 << 2
    xor ebx, 0xFFFFFFFF ; invert bits to get our mask
    and eax, ebx
    or al, 00100010b ; Set numeric error (5) monitor co-processor (1)
    mov cr0, eax
    
    ; 9: FXSAVE/FXRSTOR
    ; 7: Page Global
    ; 5: Page Address Extension
    ; 4: Page Size Extension
    ; or eax, 1 << 9 | 1 << 7 | 1 << 5 | 1 << 4
    ; 5: Page Address Extension
    ; 4: Page Size Extension
    ; or eax, 1 << 5 | 1 << 4
    ; 10: Unmasked SIMD Exceptions
    ; 9: FXSAVE/FXRSTOR
    ; 6: Machine Check Exception	
    ; 5: Page Address Extension
    ; 3: Debugging Extensions
    mov eax, cr4
    or eax, 1 << 10 | 1 << 9 | 1 << 6  | 1 << 5 | 1 << 3
    mov cr4, eax

    ; initialize floating point registers
    fninit
    
    mov eax, 6
    mov [trampoline.booting], eax

    ; enable long mode
    mov ecx, 0xC0000080               ; Read from the EFER MSR.
    rdmsr
    or eax, 1 << 11 | 1 << 14 | 1 << 8 
    wrmsr
    xor ecx, ecx
    mov eax, 7
    mov [trampoline.booting], eax
    ; enabling paging and protection simultaneously
    mov eax, cr0
    ; 31: Paging
    ; 16: Enable write protection for ring 0
    ; 5: Numeric error
    ; 4: Extension type
    ; 1: Monitor co-processor
    ; 0: Protected Mode
    or eax, 1 << 31 | 1 << 16 | 1 << 5 | 1 << 4 | 1 << 1 | 1 << 0
    ; or eax, 1 << 31 | 1 << 16 | 1 << 0
    mov cr0, eax
    lgdt [gdtr]
    jmp gdt.kernel_code:long_mode_ap

align 16

USE64
long_mode_ap:
    mov rax, gdt.kernel_data
    mov ds, rax
    mov es, rax
    mov ss, rax
    mov rax, 0x100
    wbinvd
    lock xchg [trampoline.booting], rax
    wbinvd
    mov rax, [trampoline.stack_start]
    
    mov rsp, rax
    mov rbx, [trampoline.code]
    jmp [trampoline.code]
halt_loop:
    cli
    hlt
    jmp short halt_loop;

ALIGN 16
;temporary GDT, we'll set this in code later.
gdt:
.null equ $ - gdt
    dq 0
.kernel_code equ $ - gdt
    ; 53: Long mode
    ; 47: Present
    ; 44: Code/data segment
    ; 43: Executable
    ; 41: Readable code segment
    dq 0x00209A0000000000             ; 64-bit code descriptor (exec/read).
.kernel_data equ $ - gdt
    ; 47: Present
    ; 44: Code/data segment
    ; 41: Writable data segment
    dq 0x0000920000000000             ; 64-bit data descriptor (read/write).

.end equ $ - gdt
gdtr:
    dw gdt.end - 1 ; size
    .offset dq gdt  ; offset
; temporary IDT, we'll also set this in code later.
ALIGN 16
idt:
    dw 0
    dd 0

ALIGN 16
early_stack:
times 2048 db 0
.end equ $
times 256 db 0
db trampoline.ready