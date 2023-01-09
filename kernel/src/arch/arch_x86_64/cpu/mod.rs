use core::arch::{asm, x86_64::CpuidResult};

use x86_64::{
    instructions::interrupts::{self, are_enabled},
    software_interrupt,
    structures::paging::{PageTableFlags, PhysFrame},
    PhysAddr,
};

use kernel_shared::memory::memcpy;

use crate::{
    arch::arch_x86_64::{cpu, timer::SPIN_TIMER},
    debug, error,
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
    .ready: dq 0
    .cpu_id: dq 0
    .page_table: dq 0
    .stack_start: dq 0
    .stack_end: dq 0
    .code: dq 0
    .base: dq 0
*/
const BASE_OFFSET: isize = 1;
const READY_OFFSET: isize = 0;
const CPU_ID_OFFSET: isize = 1;
const PAGE_TABLE_OFFSET: isize = 2;
const STACK_START_OFFSET: isize = 3;
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
        debug!("IPI Trampoline: Created at {:p}", page);
        let ret = Self {
            payload: page as *mut u64,
        };
        ret
    }

    fn set_base_offset(&self) {
        debug!("IPI Trampoline: Setting base offset to: {:p}", self.payload);
        self.set_value(BASE_OFFSET_OFFSET, self.payload as u64);
    }

    pub fn clone_to(&self, page: *mut u8) -> Self {
        unsafe { memcpy(page, self.payload as *const u8, 4096) };
        InterProcessorInterruptPayload::new(page)
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.payload as *const u8
    }

    pub fn load(&self, data: &[u8]) {
        unsafe {
            debug!(
                "IPI Trampoline: Load - {} bytes into {:p} from {:p}",
                data.len(),
                self.as_ptr(),
                data.as_ptr()
            );
            memcpy(self.payload as *mut u8, data.as_ptr(), data.len());
        };
        self.set_base_offset();
    }

    pub fn set_stack(&self, stack: *const u8, stack_length: usize) {
        unsafe {
            debug!(
                "IPI Trampoline: Stack: {:p}-{:p}",
                stack,
                stack.offset(stack_length as isize)
            );
            self.payload
                .offset(STACK_START_OFFSET)
                .write_volatile(stack as u64);
            self.payload
                .offset(STACK_END_OFFSET)
                .write_volatile((stack as u64) + stack_length as u64);
        }
    }

    fn set_value(&self, index: isize, val: u64) {
        unsafe {
            self.payload.offset(index + BASE_OFFSET).write_volatile(val);
        }
    }

    fn get_value(&self, index: isize) -> u64 {
        unsafe { self.payload.offset(index + BASE_OFFSET).read_volatile() }
    }

    pub fn set_cpu_id(&self, cpu_id: u64) {
        debug!("IPI Trampoline: CPU ID: {}", cpu_id);
        self.set_value(CPU_ID_OFFSET, cpu_id);
    }

    pub fn set_page_table(&self, page_table: u64) {
        debug!("IPI Trampoline: Page table: {}", page_table);
        self.set_value(PAGE_TABLE_OFFSET, page_table);
    }

    pub fn set_entry_point(&self, ap_entry: *const ()) {
        debug!("IPI Trampoline: Entry point: {:p}", ap_entry);
        self.set_value(ENTRY_ADDRESS_OFFSET, ap_entry as u64);
    }

    pub fn get_code_segment(&self) -> u16 {
        (self.as_ptr() as usize >> 12) as u16 & 0x00FFu16
    }

    pub fn get_cpu_id(&self) -> u64 {
        self.get_value(CPU_ID_OFFSET)
    }

    pub fn is_booting(&self) -> bool {
        self.boot_diag() != 0
    }

    pub fn boot_diag(&self) -> u64 {
        self.get_value(BOOTING_OFFSET)
    }

    fn clear_booting_flag(&self) {
        self.set_value(BOOTING_OFFSET, 0)
    }

    pub fn is_ready(&self) -> bool {
        self.get_value(READY_OFFSET) != 0
    }

    pub fn boot(&self) {
        core::hint::black_box(self);
        let segment = self.get_code_segment() as u8;
        debug!(
            "Sending IPI and SIPI, CS for CPU boot is: {}, for address: {:p}.",
            segment,
            self.as_ptr()
        );
        let cpu_id = self.get_cpu_id();
        self.set_base_offset();
        unsafe {
            core::hint::black_box(LOCAL_APIC);
            for i in 0..2 {
                if i != 0 {
                    error!("CPU Did not start after startup sequence, retrying.");
                }
                debug!("AP Boot attempt #{}", i);
                self.clear_booting_flag();
                LOCAL_APIC.send_ipi_init(cpu_id);
                debug!("INIT-IPI Sent");
                SPIN_TIMER.millis(10);
                for x in 0..2 {
                    if self.is_ready() {
                        break;
                    } else if x != 0 {
                        warn!(
                            "CPU {} did not report after first SIPI, trying again (Attempt #{}).",
                            cpu_id, x
                        );
                    }
                    asm!("mfence", "lfence");
                    LOCAL_APIC.send_ipi_start(cpu_id, segment);
                    asm!("mfence", "lfence");
                    debug!("START-IPI Sent");
                    match x {
                        0 => {
                            for _ in 0..200 {
                                core::hint::spin_loop();
                                SPIN_TIMER.micros(1);
                            }
                        }
                        1 => {
                            for _ in 0..1000 {
                                core::hint::spin_loop();
                                SPIN_TIMER.millis(1);
                            }
                        }
                        _ => {}
                    }
                    if self.is_booting() {
                        let mut boot_state = self.boot_diag();
                        debug!("CPU {} now running! (State: {})", cpu_id, boot_state);
                        while !self.is_ready() {
                            core::hint::spin_loop();
                            let current_state = self.boot_diag();
                            if (current_state == boot_state) {
                                continue;
                            }
                            debug!(
                                "CPU Boot state updated: {} -> {}",
                                boot_state, current_state
                            );
                            boot_state = current_state;
                        }
                        debug!("CPU {}: Boot complete!", cpu_id);
                    }
                }
            }
        }
        if !self.is_ready() {
            panic!("CPU BOOT FAILED FOR CPU: {}", self.get_cpu_id());
        }
        self.wait_for_cpu_online();
    }

    fn wait_for_cpu_online(&self) {
        while !self.is_ready() {
            core::hint::spin_loop();
        }
    }
}

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

pub fn start_additional_cpus() {
    let frame = unsafe {
        KERNEL_FRAME_ALLOCATOR
            .force_allocate(PhysFrame::containing_address(PhysAddr::new(0)))
            .expect("Unable to allocate conventional memory for IPI bootstrap trampoline!")
    };
    let frame_start_pointer = frame.start_address().as_u64() as usize as *mut u8;
    KERNEL_MEMORY_MANAGER.lock().identity_map(
        frame,
        PageTableFlags::WRITABLE | PageTableFlags::NO_CACHE | PageTableFlags::PRESENT,
    );
    debug!(
        "Identity mapping {:p}, so we don't confuse IPI when it loads the page tables",
        frame_start_pointer
    );
    let ipi_payload = InterProcessorInterruptPayload::new(frame_start_pointer);
    ipi_payload.load(BOOTSTRAP_CODE);

    unsafe {
        let platform_info = ACPI_TABLES.get().unwrap().platform_info().unwrap();
        let processor_info = platform_info.processor_info.unwrap();

        setup_trampoline_common_parameters(&ipi_payload);

        for app_cpu in processor_info.application_processors.iter() {
            debug!("Attempting to start CPU {}", app_cpu.local_apic_id);
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
    debug!("Setting up trampoline code for application CPU {}", cpu_id);
    setup_trampoline(cpu_id, &ipi_payload);
    ipi_payload.boot();
}

pub fn create_ap_stack(size: usize) -> *mut u8 {
    let pages = (size / PAGE_SIZE) + 1;
    debug!("Allocating stack space for CPU");
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
    debug!("Setting up global trampoline parameters");
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

pub fn setup_trampoline(cpu_id: usize, ipi_payload: &InterProcessorInterruptPayload) {
    debug!("Setting up trampoline for CPU {}", cpu_id);
    ipi_payload.set_cpu_id(cpu_id as u64);
    let stack_length = CPU_STACK_PAGES * PAGE_SIZE;
    let stack = create_ap_stack(stack_length);
    ipi_payload.set_stack(stack, stack_length);
    debug!("CPU Bootstrap trampoline prepared.");
}

pub extern "C" fn ap_entry(cpu_id: usize, notify_addr: *mut u64) -> ! {
    if !interrupts::are_enabled() {
        interrupts::enable();
    }
    unsafe { notify_addr.write_volatile(1) };
    crate::arch::arch_x86_64::gdt::init(cpu_id);
    crate::arch::arch_x86_64::idt::init();

    unsafe {
        asm!("mov rdx, 0xDEADBEEF", "int 254");
    }
    debug!("AP {} ONLINE!", cpu_id);
    let local_apic_id = cpu_apic_id();
    debug!("APIC ID: {}", local_apic_id);
    debug!("APIC reported ID: {}", unsafe { LOCAL_APIC.get_apic_id() });

    debug!("Enabling interrupts and spinning with hlt");
    loop {
        x86_64::instructions::interrupts::enable_and_hlt();
    }
}
