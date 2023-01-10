use core::{
    arch::asm,
    panic,
    ptr::{read_volatile, write_volatile}
};

use lazy_static::*;
use spin::{self, Mutex};
use volatile::Volatile;
use x86_64::{
    set_general_handler,
    structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode},
    VirtAddr,
};

use crate::{
    arch::arch_x86_64::{
        gdt::{CONTEXT_SWITCH_IST_INDEX, DOUBLE_FAULT_IST_INDEX},
        cpu,
    },
    debug, println, warn,
};

use super::{apic::LOCAL_APIC, gdt::INTERRUPT_STACK_SIZE};

pub mod contextswitch;

static boot_cpu_gs_base: [u8; INTERRUPT_STACK_SIZE] = [0; INTERRUPT_STACK_SIZE];

macro_rules! add_handler {
    ($idt: ident, $name: tt ) => {
        $idt.$name.set_handler_fn(InterruptHandlers::$name);
    };
    ($idt: ident, $name: tt, $stack_index: tt) => {
        unsafe {
            $idt.$name
                .set_handler_fn(InterruptHandlers::$name)
                .set_stack_index($stack_index);
        }
    };
}

struct InterruptHandlers {}

impl InterruptHandlers {
    extern "x86-interrupt" fn breakpoint(stack_frame: InterruptStackFrame) {
        println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
    }

    extern "x86-interrupt" fn double_fault(
        stack_frame: InterruptStackFrame,
        error_code: u64,
    ) -> ! {
        panic!(
            "EXCEPTION: DOUBLE FAULT on CPU {}: {}\n{:#?}",
            cpu::current(), error_code, stack_frame
        );
    }

    extern "x86-interrupt" fn page_fault(
        stack_frame: InterruptStackFrame,
        error_code: PageFaultErrorCode,
    ) {
        let virtual_address = x86_64::registers::control::Cr2::read();
        panic!(
            "Page fault in early memory manager, stack frame IP: {:#016x}, error code: {:?}\n{:?}\n\nOffending virtual address: {:?}",
            stack_frame.instruction_pointer.as_u64(),
            error_code,
            stack_frame,
            virtual_address
        );
    }
    extern "x86-interrupt" fn alignment_check(stack_frame: InterruptStackFrame, error_code: u64) {
        panic!("ALIGNMENT CHECK {}", error_code);
    }
    extern "x86-interrupt" fn bound_range_exceeded(stack_frame: InterruptStackFrame) {
        panic!("BOUND RANGE EXCEEDED");
    }
    extern "x86-interrupt" fn invalid_opcode(stack_frame: InterruptStackFrame) {
        panic!("INVALID OPCODE");
    }
    extern "x86-interrupt" fn invalid_tss(stack_frame: InterruptStackFrame, error_code: u64) {
        panic!("INVALID TSS {}", error_code);
    }

    extern "x86-interrupt" fn general_protection_fault(stack_frame: InterruptStackFrame, error_code: u64) {
        panic!("GENERAL PROTECTION FAULT {}", error_code);
    }

    extern "x86-interrupt" fn debug(stack_frame: InterruptStackFrame) {
        panic!("DEBUG");
    }

    extern "x86-interrupt" fn device_not_available(stack_frame: InterruptStackFrame) {
        panic!("DEVICE NOT AVAILABLE");
    }

    extern "x86-interrupt" fn divide_error(stack_frame: InterruptStackFrame) {
        panic!("DIVIDE ERROR");
    }

    extern "x86-interrupt" fn machine_check(stack_frame: InterruptStackFrame) -> ! {
        panic!("MACHINE CHECK");
    }

    extern "x86-interrupt" fn non_maskable_interrupt(stack_frame: InterruptStackFrame) {
        panic!("NMI");
    }

    extern "x86-interrupt" fn overflow(stack_frame: InterruptStackFrame) {
        panic!("OVERFLOW");
    }
    extern "x86-interrupt" fn security_exception(stack_frame: InterruptStackFrame, error_code: u64) {
        panic!("SECURITY EXCEPTION {}", error_code);
    }
    extern "x86-interrupt" fn segment_not_present(stack_frame: InterruptStackFrame, error_code: u64) {
        panic!("SEGMENT NOT PRESENT {}", error_code);
    }
    extern "x86-interrupt" fn simd_floating_point(stack_frame: InterruptStackFrame) {
        panic!("SIMD FLOATING POINT");
    }
    extern "x86-interrupt" fn stack_segment_fault(stack_frame: InterruptStackFrame, error_code: u64) {
        panic!("STACK SEGMENT FAULT");
    }
    extern "x86-interrupt" fn virtualization(stack_frame: InterruptStackFrame) {
        panic!("VIRTUALIZATION");
    }
    extern "x86-interrupt" fn vmm_communication_exception(stack_frame: InterruptStackFrame, error_code: u64) {
        panic!("VMM COMMUNICATION EXCEPTION");
    }
    extern "x86-interrupt" fn x87_floating_point(stack_frame: InterruptStackFrame) {
        panic!("X87 FLOATING POINT");
    }
}

lazy_static! {
    pub static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        // Interrupt handlers
        add_handler!(idt, breakpoint);
        add_handler!(idt, page_fault);
        add_handler!(idt, alignment_check);
        add_handler!(idt, bound_range_exceeded);
        add_handler!(idt, invalid_opcode);
        add_handler!(idt, invalid_tss);
        add_handler!(idt, general_protection_fault);
        add_handler!(idt, double_fault, DOUBLE_FAULT_IST_INDEX);
        add_handler!(idt, debug);
        add_handler!(idt, device_not_available);
        add_handler!(idt, divide_error);
        add_handler!(idt, machine_check);
        add_handler!(idt, non_maskable_interrupt);
        add_handler!(idt, overflow);
        add_handler!(idt, security_exception);
        add_handler!(idt, segment_not_present);
        add_handler!(idt, simd_floating_point);
        add_handler!(idt, stack_segment_fault);
        add_handler!(idt, virtualization);
        add_handler!(idt, vmm_communication_exception);
        add_handler!(idt, x87_floating_point);

        // Allocate all general handlers to our generic handler.
        unsafe {
            idt[0xFE].set_handler_fn(contextswitch::context_switch).set_stack_index(CONTEXT_SWITCH_IST_INDEX);
        }
        set_general_handler!(&mut idt, general_interrupt_handler, 0x20);
        set_general_handler!(&mut idt, general_interrupt_handler, 0xFF);
        set_general_handler!(&mut idt, general_interrupt_handler, 0x80);
        set_interrupt_handler(0x20, Some(apic_timer_interrupt_handler));
        set_interrupt_handler(0x80, Some(legacy_syscall_interrupt_handler));
        set_interrupt_handler(0xFF, Some(apic_spurious_interrupt_handler));
        idt
    };
}

pub fn init() {
    IDT.load();
}

fn apic_timer_interrupt_handler(frame: InterruptStackFrame, vector: u8, _error_code: Option<u64>) {
    unsafe {
        let ticks = read_volatile(&TICKS);
        write_volatile(&mut TICKS, ticks + 1);
        LOCAL_APIC.end_of_interrupt();
    }
}

fn apic_spurious_interrupt_handler(
    frame: InterruptStackFrame,
    vector: u8,
    _error_code: Option<u64>,
) {
    debug!("Spurious interrupt!!");
    unsafe {
        LOCAL_APIC.end_of_interrupt();
    }
}

static mut TICKS: usize = 0;

pub fn get_timer_ticks_hardware() -> usize {
    unsafe { read_volatile(&TICKS) }
}

type SoftwareInterruptHandler = fn(InterruptStackFrame, u8, Option<u64>);
fn legacy_syscall_interrupt_handler(
    stack_frame: InterruptStackFrame,
    index: u8,
    error_code: Option<u64>,
) {
    debug!(
        "Legacy syscall via interrupt ISR: {:#02x}, from RIP: {:#016x}",
        index, stack_frame.instruction_pointer
    );
}
lazy_static! {
    static ref SOFTWARE_HANDLERS: Mutex<[Option<SoftwareInterruptHandler>; 224]> =
        Mutex::new([None; 224]);
}


pub fn clear_interrupt_handler(interrupt: u8) {
    set_interrupt_handler(interrupt, None);
}
pub fn set_interrupt_handler(interrupt: u8, handler: Option<SoftwareInterruptHandler>) {
    if interrupt < 32 {
        panic!("Hardware exception interrupt {:#02x} cannot be configured with a software interrupt handler", interrupt);
    }

    let index = interrupt - 32;
    let mut handlers = SOFTWARE_HANDLERS.lock();
    handlers[index as usize] = handler;
}

fn general_interrupt_handler(stack_frame: InterruptStackFrame, index: u8, error_code: Option<u64>) {
    let handlers = SOFTWARE_HANDLERS.lock();
    let handler = handlers[(index - 32) as usize];
    if handler.is_some() {
        // debug!(
        //     "DISPATCH: {:#02x} from {:#016x}",
        //     index, stack_frame.instruction_pointer
        // );
        handler.unwrap()(stack_frame, index, error_code);
    } else {
        warn!(
            "Unable to dispatch {:#02x} from {:#016x}, no handler is defined.",
            index, stack_frame.instruction_pointer
        );
    }
}

//type HandlerFunc = extern "x86-interrupt" fn(_: InterruptStackFrame);
