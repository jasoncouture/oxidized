use core::{arch::asm, intrinsics::atomic_store_seqcst, sync::atomic};

use alloc::collections::BTreeMap;
use kernel_shared::memory::memcpy;
use lazy_static::lazy_static;
use raw_cpuid::cpuid;
use spin::Mutex;
use x86::msr::{wrmsr, IA32_X2APIC_ICR};
use x86_64::{structures::paging::PageTableFlags, PhysAddr};

const CPU_STACK_PAGES: usize = 16;

static BOOTSTRAP_CODE: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/src/arch/arch_x86_64/cpu/trampoline.bin"
));

use crate::{
    debug,
    memory::{
        allocator::{KERNEL_FRAME_ALLOCATOR, PAGE_SIZE},
        KERNEL_MEMORY_MANAGER,
    },
};

use super::{acpi::ACPI_TABLES, apic::LOCAL_APIC};

pub extern "C" fn cpu_apic_id() -> u8 {
    let mut acpi_id: u32;
    unsafe {
        asm!(
        "
         // Due to LLVM, we need to preserve (R|E|)BX, so we copy it to eax instead.
         push rbx;
         mov rax, 1; 
         cpuid; 
         mov eax, ebx
         shr eax, 24;
         pop rbx;
         ", 
            out("eax") acpi_id
        );
    }
    return acpi_id as u8;
}

pub fn start_additional_cpus(trampoline_frame: *mut u8) {
    KERNEL_MEMORY_MANAGER
        .lock()
        .identity_map_writable_data_for_kernel(PhysAddr::new(0));
    unsafe {
        let platform_info = ACPI_TABLES.get().unwrap().platform_info().unwrap();
        let processor_info = platform_info.processor_info.unwrap();

        setup_trampoline_common_parameters();

        for app_cpu in processor_info.application_processors.iter() {
            start_cpu(app_cpu.local_apic_id as usize, trampoline_frame);
        }
    }
}

fn start_cpu(cpu_id: usize, trampoline_frame: *mut u8) {
    if cpu_id == cpu_apic_id() as usize {
        panic!("Attempted to start CPU that is currently executing code");
    }
    debug!("Setting up trampoline code for application CPU {}", cpu_id);
    setup_trampoline(cpu_id, trampoline_frame);
    send_ipi_and_wait(cpu_id, trampoline_frame);
}

fn send_ipi_and_wait(cpu_id: usize, trampoline_frame: *mut u8) {
    unsafe {
        LOCAL_APIC.send_ipi_init(cpu_id as u64);
        let segment = (trampoline_frame as u64 >> 12 & 0xFF) as u8;
        LOCAL_APIC.send_ipi_start(cpu_id as u64, segment);
        debug!("Waiting for CPU {} to start", cpu_id);
    }
    wait_for_cpu_online(cpu_id, trampoline_frame);
}

fn wait_for_cpu_online(cpu_id: usize, trampoline_frame: *mut u8) {
    let state_pointer = unsafe { (trampoline_frame as *mut u64).offset(1) };
    while unsafe { state_pointer.read_volatile() } == 0 {
        core::hint::spin_loop();
    }
}

pub fn create_ap_stack(size: usize) -> *const u8 {
    let pages = (size / PAGE_SIZE) + 1;

    let mut locked_manager = KERNEL_MEMORY_MANAGER.lock();
    locked_manager
        .allocate_contigious_address_range(
            pages,
            None,
            PageTableFlags::WRITABLE | PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE,
        )
        .expect("Unable to allocate CPU Stack space!") as *const u8
}
/*
trampoline:
    jmp short startup_ap
    times 8 - ($ - trampoline) nop
    .ready: dq 0
    .cpu_id: dq 0
    .page_table: dq 0
    .stack_start: dq 0
    .stack_end: dq 0
    .code: dq 0
*/
pub fn get_trampoline_parameters() -> &'static mut TrampolineParameters {
    unsafe {
        (BOOTSTRAP_CODE.as_ptr() as *mut TrampolineParameters)
            .as_mut()
            .unwrap()
    }
}

pub fn setup_trampoline_common_parameters() {
    let parameters = get_trampoline_parameters();
    parameters.code = ap_entry as *const () as usize;
    unsafe {
        asm!(
            "mov rax, cr3",
            out("rax") parameters.page_table
        )
    }
}

#[derive(Debug)]
#[repr(C, align(8))]
pub struct TrampolineParameters {
    reserved: usize,
    ready: usize,
    cpu_id: usize,
    page_table: usize,
    stack_start: usize,
    stack_end: usize,
    code: usize,
}

pub fn setup_trampoline(cpu_id: usize, trampoline_frame: *mut u8) {
    let parameters = get_trampoline_parameters();
    parameters.cpu_id = cpu_id;
    let stack = create_ap_stack(CPU_STACK_PAGES * PAGE_SIZE);
    set_ap_init_stack(stack, CPU_STACK_PAGES * PAGE_SIZE);

    let ap_ready = unsafe { (BOOTSTRAP_CODE.as_ptr() as *mut usize).offset(1) }; // The first 8 bytes are used to jump to the init code. So skip it.
    let ap_cpu_id = unsafe { ap_ready.offset(1) };

    unsafe { atomic_store_seqcst(ap_ready, 0) };
    unsafe { atomic_store_seqcst(ap_cpu_id, cpu_id) };
    copy_ap_init(trampoline_frame); // Copy trampoline init, with it's data to init page.
}

pub fn set_ap_init_stack(stack: *const u8, pages: usize) {
    unsafe {
        let template = get_trampoline_parameters() as *mut TrampolineParameters;
        let stack_end = stack.byte_offset((pages * PAGE_SIZE) as isize);
        let ap_stack_start = (template as *mut usize).offset(4);
        let ap_stack_end = ap_stack_start.offset(1);
        atomic_store_seqcst(ap_stack_start, stack as usize);
        atomic_store_seqcst(ap_stack_end, stack_end as usize);
    }
}

pub fn copy_ap_init(trampoline_frame: *mut u8) {
    unsafe {
        memcpy(trampoline_frame, BOOTSTRAP_CODE.as_ptr(), 0x1000);
    }
}

pub extern "C" fn ap_entry(cpu_id: usize) -> ! {
    debug!("Initialized CPU {} successfully, parking.", cpu_id);
    loop {
        x86_64::instructions::interrupts::enable_and_hlt();
    }
}
