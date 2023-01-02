pub mod contextswitch;

use core::{
    panic,
    ptr::{read_volatile, write_volatile}, arch::asm,
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
    arch::arch_x86_64::{gdt::{DOUBLE_FAULT_IST_INDEX, CONTEXT_SWITCH_IST_INDEX}, idt::contextswitch::_context_switch},
    debug, println, warn,
};

use super::{apic::LOCAL_APIC, gdt::INTERRUPT_STACK_SIZE};

static boot_cpu_gs_base: [u8; INTERRUPT_STACK_SIZE] = [0; INTERRUPT_STACK_SIZE];

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        // Interrupt handlers
        idt.breakpoint.set_handler_fn(InterruptHandlers::breakpoint_handler);
        idt.page_fault.set_handler_fn(InterruptHandlers::page_fault_handler);
        unsafe {
            idt.double_fault.set_handler_fn(InterruptHandlers::double_fault_handler).set_stack_index(DOUBLE_FAULT_IST_INDEX);
        }

        // Allocate all general handlers to our generic handler.
        unsafe {
            idt[0xFE].set_handler_addr(VirtAddr::new(_context_switch as u64)).set_stack_index(CONTEXT_SWITCH_IST_INDEX);
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

struct InterruptHandlers {}

impl InterruptHandlers {
    extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
        println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
    }

    extern "x86-interrupt" fn double_fault_handler(
        stack_frame: InterruptStackFrame,
        error_code: u64,
    ) -> ! {
        panic!(
            "EXCEPTION: DOUBLE FAULT: {}\n{:#?}",
            error_code, stack_frame
        );
    }

    extern "x86-interrupt" fn page_fault_handler(
        stack_frame: InterruptStackFrame,
        error_code: PageFaultErrorCode,
    ) {
        panic!(
            "Page fault in early memory manager, stack frame IP: {:#016x}, error code: {:?}\n{:?}",
            stack_frame.instruction_pointer.as_u64(),
            error_code,
            stack_frame
        );
    }
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
