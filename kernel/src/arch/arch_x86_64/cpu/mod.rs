use core::{alloc::Layout, arch::asm, cell::OnceCell};

use alloc::{format, string::String};
use bitvec::array::BitArray;
use bitvec::prelude::*;

use iced_x86::{Decoder, DecoderOptions, Formatter, Instruction, Mnemonic, NasmFormatter};
use spin::Mutex;
use x86::msr::{rdmsr, IA32_EFER};
use x86_64::{
    instructions::interrupts,
    registers::{control::{Cr0, Cr4, Cr4Flags, Cr0Flags}, model_specific::{EferFlags, Efer}},
    structures::paging::{PageTableFlags, PhysFrame},
    PhysAddr,
};

use kernel_shared::memory::memcpy;

use crate::kernel_cpu_main;
use crate::{
    arch::arch_x86_64::{apic, gdt, idt},
    memory::allocator::kmalloc,
};
use crate::{
    debug,
    memory::{
        allocator::{KERNEL_FRAME_ALLOCATOR, PAGE_SIZE},
        KERNEL_MEMORY_MANAGER,
    },
};

use super::{acpi::ACPI_TABLES, apic::LOCAL_APIC};

pub(crate) const CPU_STACK_PAGES: usize = 256;

static BOOTSTRAP_CODE: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/src/arch/arch_x86_64/cpu/trampoline.bin"
));

static mut BSP_EFER: u64 = 0;
static mut BSP_CR0: u64 = 0;
static mut BSP_CR4: u64 = 0;

/*
trampoline:
    .page_table: dq 0 ; -2
    .stack_end: dq 0 ; -1
    .code: dq 0 ; 0
*/
const PAGE_TABLE_OFFSET: isize = -3;
const STACK_END_OFFSET: isize = -2;
const ENTRY_ADDRESS_OFFSET: isize = -1;

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

    pub fn as_ptr(&self) -> *const u8 {
        self.payload as *const u8
    }

    pub fn load(&self, data: &[u8]) {
        unsafe {
            memcpy(self.payload as *mut u8, data.as_ptr(), data.len());
        };
    }

    pub fn set_stack(&self, stack: *const u8, stack_length: usize) {
        let stack_end = unsafe { stack.offset(stack_length as isize) };
        if !stack.is_aligned_to(16) || !stack_end.is_aligned_to(16) {
            panic!(
                "Attempted to start a CPU with an unaligned stack! {:p}-{:p}",
                stack, stack_end
            );
        }
        self.set_value(STACK_END_OFFSET, (stack as u64) + stack_length as u64);
    }

    fn dump_assembly(&self) {
        let mut buffer = [0u8; 4096];
        const HEXBYTES_COLUMN_BYTE_LENGTH: usize = 10;
        unsafe { memcpy(buffer.as_mut_ptr(), self.as_ptr(), BOOTSTRAP_CODE.len()) };
        let buffer = &buffer[0..BOOTSTRAP_CODE.len()];
        let mut decoder = Decoder::with_ip(16, buffer, 0, DecoderOptions::NONE);

        // Formatters: Masm*, Nasm*, Gas* (AT&T) and Intel* (XED).
        // For fastest code, see `SpecializedFormatter` which is ~3.3x faster. Use it if formatting
        // speed is more important than being able to re-assemble formatted instructions.
        let mut formatter = NasmFormatter::new();

        // Change some options, there are many more
        formatter.options_mut().set_digit_separator("`");
        formatter.options_mut().set_first_operand_char_index(10);

        // String implements FormatterOutput
        let mut output = String::new();

        let mut instruction = Instruction::default();

        while decoder.can_decode() {
            // There's also a decode() method that returns an instruction but that also
            // means it copies an instruction (40 bytes):
            //     instruction = decoder.decode();
            decoder.decode_out(&mut instruction);
            // The first jump we hit in 16 bit mode, is the jump to 64 bit mode.
            // Update the decoder accordingly.
            if instruction.code().mnemonic() == Mnemonic::Jmp && decoder.bitness() == 16 {
                decoder = Decoder::with_ip(64, buffer, 0, DecoderOptions::NONE);
                let rip = instruction.ip() as usize + instruction.len();
                decoder.set_position(rip).unwrap();
                decoder.set_ip(rip as u64);
            }

            // Format the instruction ("disassemble" it)
            output.clear();
            formatter.format(&instruction, &mut output);
            let mut final_output: String = String::new();
            final_output.push_str(format!("{:016X}: ", instruction.ip()).as_str());
            let start_index = (instruction.ip()) as usize;
            let instr_bytes = &buffer[start_index..start_index + instruction.len()];
            for b in instr_bytes.iter() {
                final_output.push_str(format!("{:02X}", b).as_str());
            }
            if instr_bytes.len() < HEXBYTES_COLUMN_BYTE_LENGTH {
                for _ in 0..HEXBYTES_COLUMN_BYTE_LENGTH - instr_bytes.len() {
                    final_output.push_str("  ");
                }
            }
            final_output.push_str(format!(" {}", output).as_str());
            debug!("{}", final_output);
            //debug!("{:016X} {}", instruction.ip(), output);
        }
    }

    fn set_value(&self, index: isize, val: u64) {
        unsafe {
            let end = (self.payload as *mut u8).offset(BOOTSTRAP_CODE.len() as isize) as *mut u64;
            let target = end.offset(-1).offset(index);
            target.write_volatile(val);
        }
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

    pub fn is_ready(&self, cpu_id: usize) -> bool {
        let mutex = get_online_cpu_status_bits();
        let status_bits = mutex.lock();
        let cpu_id = cpu_id;
        let result = match status_bits.get(cpu_id).as_deref() {
            Some(v) => *v,
            None => false,
        };
        result
    }

    pub fn boot(&self, cpu_id: usize) {
        let segment = self.get_code_segment() as u8;
        unsafe {
            //self.dump_assembly();
            LOCAL_APIC.send_ipi_init(cpu_id);
            debug!("IPI INIT  -> ID: {}", cpu_id);
            LOCAL_APIC.send_ipi_start(cpu_id, segment);
            debug!("IPI START -> ID: {} CS: {}", cpu_id, segment);
            while !self.is_ready(cpu_id) {
                core::hint::spin_loop();
            }

            debug!("CPU {} Signaled ready", cpu_id);
        }
    }
}

pub extern "C" fn cpu_apic_id() -> usize {
    unsafe {
        let mut id: usize;
        asm!(
            "push rbx",
            "mov rax, 1",
            "cpuid",
            "mov rax, rbx",
            "shr rax, 24",
            "and rax, 0xff",
            "pop rbx",
            out("rax") id,
            out("rcx") _,
            out("rdx") _
        );

        id
    }
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

    // All CPUs are online. Let's free our page now.
    // TODO: Implement ability to free virtual pages, so we can free the underlying frame.
    //KERNEL_MEMORY_MANAGER.lock().free_page(VirtAddr::new(frame.start_address().as_u64()));
    //unsafe { KERNEL_FRAME_ALLOCATOR.free(frame.start_address()) };
}

fn start_cpu(cpu_id: usize, ipi_payload: &InterProcessorInterruptPayload) {
    if cpu_id == cpu_apic_id() as usize {
        panic!("Attempted to start CPU that is currently executing code");
    }
    setup_trampoline(&ipi_payload);
    ipi_payload.boot(cpu_id);
}

pub fn create_ap_stack(size: usize) -> *mut u8 {
    kmalloc(Layout::from_size_align(size, 16).unwrap())
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

        BSP_EFER = rdmsr(IA32_EFER);
        BSP_CR0 = Cr0::read_raw();
        BSP_CR4 = Cr4::read_raw();
    }
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

pub fn setup_trampoline(ipi_payload: &InterProcessorInterruptPayload) {
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
    // Make sure interrupts are disabled.
    interrupts::disable();
    mark_cpu_booting();
    set_control_regs();
    gdt::init();
    idt::init();
    apic::init_ap();
    ap_main();
}

unsafe fn set_control_regs() {
    // Set EFER, CR0, and CR4 to match the BSP.
    let cr4 = Cr4Flags::from_bits_truncate(BSP_CR4);
    let cr0 = Cr0Flags::from_bits_truncate(BSP_CR0);
    let efer = EferFlags::from_bits_truncate(BSP_EFER);
    Cr4::write(cr4);
    Efer::write(efer);
    Cr0::write(cr0);
}

pub fn ap_main() -> ! {
    mark_cpu_online();
    interrupts::enable();
    kernel_cpu_main();
}

pub fn current() -> usize {
    cpu_apic_id()
}
