use core::{arch::asm, cell::OnceCell};

use alloc::slice;
use bitvec::prelude::*;
use bitvec::{array::BitArray, ptr::slice_from_raw_parts};

use spin::Mutex;
use x86_64::{
    software_interrupt,
    structures::paging::{PageTableFlags, PhysFrame},
    PhysAddr,
};

use kernel_shared::memory::memcpy;

use crate::kernel_cpu_main;
use crate::{
    arch::arch_x86_64::timer::SPIN_TIMER,
    debug,
    memory::{
        allocator::{KERNEL_FRAME_ALLOCATOR, PAGE_SIZE},
        KERNEL_MEMORY_MANAGER,
    },
    warn,
};

use super::{acpi::ACPI_TABLES, apic::LOCAL_APIC};

const CPU_STACK_PAGES: usize = 16;

static BOOTSTRAP_CODE: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/src/arch/arch_x86_64/cpu/trampoline.bin"
));

/*
trampoline:
    jmp short startup_ap
    times 8 - ($ - trampoline) nop
    .ready: dq 0 ;0
    .cpu_id: dq 0 ;1
    .page_table: dq 0 ;2
    .stack_start: dq 0 ;3
    .stack_end: dq 0 ;4
    .code: dq 0 ;5
    .booting: dq 0 ;6
*/
const BASE_OFFSET: isize = 1;
const CPU_ID_OFFSET: isize = 1;
const PAGE_TABLE_OFFSET: isize = 2;
const STACK_END_OFFSET: isize = 4;
const ENTRY_ADDRESS_OFFSET: isize = 5;
const BASE_OFFSET_OFFSET: isize = 6;
const BOOTING_OFFSET: isize = 7;

#[repr(C)]
pub struct InterProcessorInterruptPayload {
    payload: *mut u64,
}

impl InterProcessorInterruptPayload {
    pub fn new(page: *mut u8) -> Self {
        let ret = Self {
            payload: page as *mut u64,
        };
        ret
    }

    fn set_base_offset(&self) {
        self.set_value(BASE_OFFSET_OFFSET, self.payload as u64);
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.payload as *const u8
    }

    pub fn load(&self, data: &[u8]) {
        unsafe {
            memcpy(self.payload as *mut u8, data.as_ptr(), data.len());
        };
        self.set_base_offset();
    }

    pub fn set_stack(&self, stack: *const u8, stack_length: usize) {
        let stack_end = unsafe { stack.offset(stack_length as isize) };
        if !stack.is_aligned_to(16) || !stack_end.is_aligned_to(16) {
            panic!(
                "Attempted to start a CPU with an unaligned stack! {:p}-{:p}",
                stack, stack_end
            );
        }
        unsafe {
            self.set_value(STACK_END_OFFSET, (stack as u64) + stack_length as u64);
        }
    }

    fn set_value(&self, index: isize, val: u64) {
        unsafe {
            let target = self.payload.offset(index + BASE_OFFSET);
            asm! (
                "wbinvd",
                "lock xchg [{}], {}",
                "wbinvd",
                in(reg) target,
                in(reg) val
            );

            //debug!("{:?}", core::slice::from_raw_parts(self.payload.offset(1), 16));
        }
    }

    fn get_value(&self, index: isize) -> u64 {
        unsafe {
            let target = self.payload.offset(index + BASE_OFFSET);
            let mut val: u64 = 0;
            asm! (
                "wbinvd",
                "lock xadd [{}], {}",
                "wbinvd",
                in(reg) target,
                inout(reg) val
            );
            val
        }
    }

    pub fn set_cpu_id(&self, cpu_id: u64) {
        self.set_value(CPU_ID_OFFSET, cpu_id);
    }

    pub fn set_page_table(&self, page_table: u64) {
        self.set_value(PAGE_TABLE_OFFSET, page_table);
    }

    pub fn set_entry_point(&self, ap_entry: *const ()) {
        self.set_value(ENTRY_ADDRESS_OFFSET, ap_entry as u64);
    }

    pub fn get_code_segment(&self) -> u16 {
        (self.as_ptr() as usize >> 12) as u16 & 0x00FFu16
    }

    pub fn get_cpu_id(&self) -> usize {
        self.get_value(CPU_ID_OFFSET) as usize
    }

    pub fn is_booting(&self) -> bool {
        let mutex = get_booting_cpu_status_bits();
        let status_bits = mutex.lock();
        let cpu_id = self.get_cpu_id();
        let result = match status_bits.get(cpu_id as usize).as_deref() {
            Some(v) => *v,
            None => false,
        };
        result
    }

    fn clear_booting_flag(&self) {
        self.set_value(BOOTING_OFFSET, 0)
    }

    pub fn is_ready(&self) -> bool {
        let mutex = get_online_cpu_status_bits();
        let status_bits = mutex.lock();
        let cpu_id = self.get_cpu_id();
        let result = match status_bits.get(cpu_id as usize).as_deref() {
            Some(v) => *v,
            None => false,
        };
        result
    }

    pub fn boot(&self) {
        core::hint::black_box(self);
        let segment = self.get_code_segment() as u8;
        let cpu_id = self.get_cpu_id();
        unsafe {
            self.clear_booting_flag();
            LOCAL_APIC.send_ipi_init(cpu_id);
            debug!("IPI INIT  -> ID: {}", cpu_id);
            LOCAL_APIC.send_ipi_start(cpu_id, segment);
            debug!("IPI START -> ID: {} CS: {}", cpu_id, segment);
            while !self.is_ready() {
                core::hint::spin_loop();
            }

            debug!("CPU {} Signaled ready", cpu_id);
        }
    }
}

pub extern "C" fn cpu_apic_id() -> usize {
    unsafe { return LOCAL_APIC.get_apic_id() as usize }
}

pub fn start_additional_cpus() {
    let frame = unsafe {
        KERNEL_FRAME_ALLOCATOR
            .force_allocate(PhysFrame::containing_address(PhysAddr::new(0)))
            .expect("Unable to allocate conventional memory for IPI bootstrap trampoline!")
    };
    let frame_start_pointer = frame.start_address().as_u64() as usize as *mut u8;
    KERNEL_MEMORY_MANAGER
        .lock()
        .identity_map(frame, PageTableFlags::WRITABLE | PageTableFlags::PRESENT);
    let ipi_payload = InterProcessorInterruptPayload::new(frame_start_pointer);
    ipi_payload.load(BOOTSTRAP_CODE);

    get_online_cpu_status_bits()
        .get_mut()
        .set(cpu_apic_id() as usize, true);

    unsafe {
        let platform_info = ACPI_TABLES.get().unwrap().platform_info().unwrap();
        let processor_info = platform_info.processor_info.unwrap();

        for app_cpu in processor_info.application_processors.iter() {
            start_cpu(app_cpu.local_apic_id as usize, &ipi_payload);
        }
    }

    unsafe {
        KERNEL_FRAME_ALLOCATOR.free(frame.start_address());
    }

    // All CPUs are online. Let's free our page now.
    // TODO: Implement ability to free virtual pages, so we can free the underlying frame.
    //KERNEL_MEMORY_MANAGER.lock().free_page(VirtAddr::new(frame.start_address().as_u64()));
    //unsafe { KERNEL_FRAME_ALLOCATOR.free(frame.start_address()) };
}

fn start_cpu(cpu_id: usize, ipi_payload: &InterProcessorInterruptPayload) {
    if cpu_id == cpu_apic_id() as usize {
        panic!("Attempted to start CPU that is currently executing code");
    }
    setup_trampoline(cpu_id, &ipi_payload);
    ipi_payload.boot();
}

pub fn create_ap_stack(size: usize) -> *mut u8 {
    let pages = (size / PAGE_SIZE) + 1;
    let mut locked_manager = KERNEL_MEMORY_MANAGER.lock();
    locked_manager
        .allocate_contigious_address_range(
            pages,
            None,
            PageTableFlags::WRITABLE | PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE,
        )
        .expect("Unable to allocate CPU Stack space!")
}

pub fn setup_trampoline_common_parameters(ipi_code: &InterProcessorInterruptPayload) {
    unsafe {
        let mut page_table: u64 = 0;
        asm!(
            "mov {}, cr3",
            out(reg) page_table
        );
        ipi_code.set_page_table(page_table);
        ipi_code.set_entry_point(ap_entry as *const ());
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

static mut CPU_ONLINE_STATUS_BITS: OnceCell<Mutex<BitArray>> = OnceCell::new();
static mut CPU_BOOTING_STATUS_BITS: OnceCell<Mutex<BitArray>> = OnceCell::new();

pub fn get_online_cpu_status_bits() -> &'static mut Mutex<BitArray> {
    unsafe {
        CPU_ONLINE_STATUS_BITS.get_or_init(|| Mutex::new(bitarr!(512)));
        CPU_ONLINE_STATUS_BITS.get_mut().unwrap()
    }
}

pub fn get_booting_cpu_status_bits() -> &'static mut Mutex<BitArray> {
    unsafe {
        CPU_BOOTING_STATUS_BITS.get_or_init(|| Mutex::new(bitarr!(512)));
        CPU_BOOTING_STATUS_BITS.get_mut().unwrap()
    }
}

pub fn setup_trampoline(cpu_id: usize, ipi_payload: &InterProcessorInterruptPayload) {
    ipi_payload.set_cpu_id(cpu_id as u64);
    let stack_length = CPU_STACK_PAGES * PAGE_SIZE;
    let stack = create_ap_stack(stack_length);
    ipi_payload.set_stack(stack, stack_length);
    setup_trampoline_common_parameters(&ipi_payload);
}

fn mark_cpu_online() {
    let mutex = get_online_cpu_status_bits();
    let status_bits = mutex.get_mut();
    let local_apic_id = cpu_apic_id();
    status_bits.set(local_apic_id.into(), true);
}

fn mark_cpu_booting() {
    let mutex = get_booting_cpu_status_bits();
    let status_bits = mutex.get_mut();
    let local_apic_id = cpu_apic_id();
    status_bits.set(local_apic_id.into(), true);
}

pub unsafe extern "C" fn ap_entry() -> ! {
    mark_cpu_booting();
    debug!("AP booting.");
    debug!("Initializing GDT");
    crate::arch::arch_x86_64::gdt::init();
    crate::arch::arch_x86_64::idt::init();
    mark_cpu_online();
    debug!("AP active!");
    ap_main()
}

pub fn ap_main() -> ! {
    debug!("Testing interrupt -> 254");
    unsafe {
        software_interrupt!(254);
    }
    debug!("Resumed after interrupt.");
    kernel_cpu_main();
}

pub fn current() -> usize {
    cpu_apic_id()
}
