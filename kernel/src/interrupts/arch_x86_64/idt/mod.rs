use lazy_static::*;
use pic8259::ChainedPics;
use spin;
use x86_64::{
    instructions::interrupts,
    structures::{
        gdt,
        idt::{InterruptDescriptorTable, InterruptStackFrame},
    },
};

use crate::{interrupts::arch_x86_64::{gdt::DOUBLE_FAULT_IST_INDEX, PICS}, println};

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        // Interrupt handlers
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        unsafe {
        idt.double_fault.set_handler_fn(double_fault_handler).set_stack_index(DOUBLE_FAULT_IST_INDEX);
        }
        idt
    };
}
pub fn init() {
    IDT.load();
}

//type HandlerFunc = extern "x86-interrupt" fn(_: InterruptStackFrame);

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT: {}\n{:#?}", error_code, stack_frame);
}
