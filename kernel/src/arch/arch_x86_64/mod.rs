use alloc::string::{String, ToString};

use bootloader_api::BootInfo;
use lazy_static::lazy_static;
use pic8259::ChainedPics;
use x86::cpuid::CpuId;
use x86_64::instructions::interrupts;

use crate::{arch::arch_x86_64::cpu::start_additional_cpus, debug};

use self::cpu::cpu_apic_id;

pub(crate) mod acpi;
pub(crate) mod apic;
pub(crate) mod cpu;
pub(crate) mod gdt;
pub(crate) mod idt;
pub(crate) mod syscall;
pub(crate) mod timer;

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub fn init_common() {}

pub fn init_hardware(boot_info: &BootInfo) {
    debug!("Initializing GDT");
    gdt::init();
    debug!("Initializing IDT");
    idt::init();
    debug!("Initializing delay loops");
    timer::init();
    debug!("Initializing ACPI");
    acpi::init(boot_info.rsdp_addr.into_option());
    debug!("Initializing APIC");
    apic::init();
    start_additional_cpus();

    debug!("Initializing syscalls");
    syscall::init();
}

fn pic_init() {
    unsafe {
        debug!(
            "Remapping PIC1 and 2 interrupts offsets to {} and {}",
            PIC_1_OFFSET, PIC_2_OFFSET
        );
        let mut pics = ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET);
        pics.initialize(); // initialize the pics, and immediately disable them.
        pics.disable();
        debug!("8529 PICs have been disabled successfully.");
    }
}

pub fn breakpoint_hardware() {
    x86_64::instructions::interrupts::int3();
}

lazy_static! {
    static ref CPU_ID: CpuId = CpuId::default();
}

pub fn get_cpu_vendor_string() -> String {
    let processor_vendor_struct = CPU_ID.get_vendor_info().expect(
        "This kernel requires CPUID support, but was unable to retrieve the processor vendor",
    );
    processor_vendor_struct.as_str().to_string()
}
pub fn get_cpu_brand_string() -> String {
    let processor_brand_struct = CPU_ID.get_processor_brand_string().expect(
        "This kernel requires CPUID support, but was unable to retrieve the processor brand",
    );
    processor_brand_struct.as_str().to_string()
}

pub fn enable_interrupts_hardware() {
    interrupts::enable();
}

pub fn wait_for_interrupt_hardware() {
    interrupts::enable_and_hlt();
}

pub fn current_cpu() -> u16 {
    cpu_apic_id()
}
