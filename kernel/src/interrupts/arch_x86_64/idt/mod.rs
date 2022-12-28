use core::panic;

use lazy_static::*;
use pic8259::ChainedPics;
use spin::{self, Mutex};
use x86_64::{
    instructions::interrupts,
    structures::{
        gdt,
        idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode},
    }, set_general_handler,
};

use crate::{interrupts::arch_x86_64::{gdt::DOUBLE_FAULT_IST_INDEX, PICS}, println, info, debug, warn};

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
        set_general_handler!(&mut idt, general_interrupt_handler);
        set_general_handler!(&mut idt, general_interrupt_handler, 0x80);
        set_interrupt_handler(0x80, Some(legacy_syscall_interrupt_handler));
        idt
    };
}
pub fn init() {
    IDT.load();
}

type SoftwareInterruptHandler = fn(InterruptStackFrame, u8, Option<u64>);
fn legacy_syscall_interrupt_handler(stack_frame: InterruptStackFrame, index: u8, error_code: Option<u64>) {
    debug!("Legacy syscall via interrupt ISR: {:#02x}, from RIP: {:#016x}", index, stack_frame.instruction_pointer);
}
lazy_static! {
    static ref SOFTWARE_HANDLERS: Mutex<[Option<SoftwareInterruptHandler>; 224]> = Mutex::new( [None; 224]);
}

struct InterruptHandlers {

}

impl InterruptHandlers {
    extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
        println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
    }
    
    extern "x86-interrupt" fn double_fault_handler(
        stack_frame: InterruptStackFrame,
        error_code: u64,
    ) -> ! {
        panic!("EXCEPTION: DOUBLE FAULT: {}\n{:#?}", error_code, stack_frame);
    }
    
    extern "x86-interrupt" fn page_fault_handler(stack_frame: InterruptStackFrame, error_code: PageFaultErrorCode) {
        panic!("Page fault in early memory manager, stack frame IP: {:#016x}, error code: {:?}", stack_frame.instruction_pointer.as_u64(), error_code);
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
        debug!("DISPATCH: {:#02x} from {:#016x}", index, stack_frame.instruction_pointer);
        handler.unwrap()(stack_frame, index, error_code);
    } else {
        warn!("Unable to dispatch {:#02x} from {:#016x}, no handler is defined.", index, stack_frame.instruction_pointer);
    }
}

//type HandlerFunc = extern "x86-interrupt" fn(_: InterruptStackFrame);

