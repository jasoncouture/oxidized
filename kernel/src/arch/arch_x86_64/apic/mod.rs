use acpi::InterruptModel::*;
use alloc::string::{String, ToString};
use x86_64::PhysAddr;

use crate::{debug, memory::KERNEL_MEMORY_MANAGER};

use super::acpi::get_acpi_tables;

const APIC_REGISTER_MASK: usize = 0x0FF;
const APIC_REGISTER_ADDRESS_MASK: usize = 0x0FF0;

const APIC_REGISTER_OFFSET_ID: usize = 0x020;
const APIC_REGISTER_OFFSET_VERSION: usize = 0x030;
const APIC_REGISTER_OFFSET_SPURIOUS_INTERRUPT_VECTOR: usize = 0x0F0;
const APIC_REGISTER_OFFSET_TASK_PRIORITY: usize = 0x080;
const APIC_REGISTER_OFFSET_ARBITRATION_PRIORITY: usize = 0x090;
const APIC_REGISTER_OFFSET_PROCESSOR_PRIORITY: usize = 0x0A0;
const APIC_REGISTER_OFFSET_LOCAL_VECTOR_TABLE_TIMER: usize = 0x320;
const APIC_REGISTER_OFFSET_TIMER_DIVISOR: usize = 0x3E0;
const APIC_REGISTER_OFFSET_TIMER_INITIAL_COUNT: usize = 0x380;
const APIC_REGISTER_OFFSET_END_OF_INTERRUPT: usize = 0x0B0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct AdvancedProgrammableInterruptController {
    address: *mut u8,
}

impl AdvancedProgrammableInterruptController {
    fn read_register(self, register: usize) -> u32 {
        let register_address = register & APIC_REGISTER_ADDRESS_MASK;
        let pointer = ((self.address as usize) + register_address) as *mut u32;
        unsafe {
            let result = pointer.read_volatile();
            result
        }
    }

    fn write_register(self, register: usize, value: u32) {
        let register_address = register & APIC_REGISTER_ADDRESS_MASK;
        let pointer = ((self.address as usize) + register_address) as *mut u32;
        unsafe {
            pointer.write_volatile(value);
        }
    }

    pub fn get_apic_id(self) -> u32 {
        self.read_register(APIC_REGISTER_OFFSET_ID)
    }
    pub fn set_apic_id(self, value: u32) {
        self.write_register(APIC_REGISTER_OFFSET_ID, value);
    }

    pub fn get_version(self) -> u32 {
        self.read_register(APIC_REGISTER_OFFSET_VERSION)
    }

    pub fn get_task_priority(self) -> u32 {
        self.read_register(APIC_REGISTER_OFFSET_TASK_PRIORITY)
    }

    pub fn get_arbiration_priority(self) -> u32 {
        self.read_register(APIC_REGISTER_OFFSET_ARBITRATION_PRIORITY)
    }

    pub fn get_processor_priority(self) -> u32 {
        self.read_register(APIC_REGISTER_OFFSET_PROCESSOR_PRIORITY)
    }

    pub fn get_spurious_interrupt_vector(self) -> u32 {
        self.read_register(APIC_REGISTER_OFFSET_SPURIOUS_INTERRUPT_VECTOR)
    }
    pub fn set_spurious_interrupt_vector(self, value: u32) {
        self.write_register(APIC_REGISTER_OFFSET_SPURIOUS_INTERRUPT_VECTOR, value)
    }
    pub fn get_local_vector_table_timer(self) -> u32 {
        self.read_register(APIC_REGISTER_OFFSET_LOCAL_VECTOR_TABLE_TIMER)
    }

    pub fn set_local_vector_table_timer(self, value: u32) {
        self.write_register(APIC_REGISTER_OFFSET_LOCAL_VECTOR_TABLE_TIMER, value);
    }

    pub fn get_timer_divisor(self) -> u32 {
        self.read_register(APIC_REGISTER_OFFSET_TIMER_DIVISOR)
    }

    pub fn set_timer_divisor(self, value: u32) {
        self.write_register(APIC_REGISTER_OFFSET_TIMER_DIVISOR, value);
    }

    pub fn set_timer_initial_count(self, value: u32) {
        self.write_register(APIC_REGISTER_OFFSET_TIMER_INITIAL_COUNT, value);
    }

    pub fn end_of_interrupt(&self) {
        self.write_register(APIC_REGISTER_OFFSET_END_OF_INTERRUPT, 0);
    }
}

pub(crate) static mut LOCAL_APIC: AdvancedProgrammableInterruptController =
    AdvancedProgrammableInterruptController {
        address: 0 as *mut u8,
    };

pub fn init() {
    let acpi = get_acpi_tables();
    let platform_info = match acpi.platform_info() {
        Ok(v) => v,
        _ => panic!("Unable to retrieve MADT from ACPI?"),
    };
    let apic_info = match platform_info.interrupt_model {
        Apic(a) => a,
        _ => panic!("APIC is required by this kernel."),
    };
    super::pic_init();
    let addr = apic_info.local_apic_address;
    debug!("Local APIC address: {:p}", addr as usize as *const ());
    KERNEL_MEMORY_MANAGER
        .lock()
        .identity_map_writable_data_for_kernel(PhysAddr::new_truncate(addr));
    let apic_ptr: *mut u8 = addr as *mut u8;
    unsafe {
        LOCAL_APIC.address = apic_ptr;
    }
    debug!("Identity mapped apic into kernel memory map.");

    let id = unsafe { LOCAL_APIC.get_apic_id() };
    let version = unsafe { LOCAL_APIC.get_version() };

    debug!("APIC ID: {}, Version: {:#08x}", id, version);

    debug!("Priorities:");
    debug!("   Task       : {}", unsafe {
        LOCAL_APIC.get_task_priority()
    });
    debug!("   Arbitration: {}", unsafe {
        LOCAL_APIC.get_arbiration_priority()
    });
    debug!("   Processor  : {}", unsafe {
        LOCAL_APIC.get_processor_priority()
    });
    debug!("Setting spurious interrupt vector to 255");
    unsafe {
        let mut sivr = LOCAL_APIC.get_spurious_interrupt_vector();
        debug!("Before setting vector and enabling: {:#08x}", sivr);
        sivr = sivr | 0xFF;
        LOCAL_APIC.set_spurious_interrupt_vector(sivr);
        sivr = LOCAL_APIC.get_spurious_interrupt_vector();
        debug!("After setting vector only: {:#08x}", sivr);
        sivr = sivr | 0x100;
        LOCAL_APIC.set_spurious_interrupt_vector(sivr);
        debug!("After enabling interrupts: {:#08x}", sivr);
        debug!("Starting timer on IRQ0 (Vector 32)");
        // 0x20000 - Enable periodic timer, 32 == interrupt vector (IRQ0, Vector 32)
        LOCAL_APIC.set_local_vector_table_timer(32 | 0x20000);
        LOCAL_APIC.set_timer_divisor(0x03);
        LOCAL_APIC.set_timer_initial_count(0xFF00);
    }
}
