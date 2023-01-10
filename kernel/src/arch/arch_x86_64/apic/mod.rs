use alloc::string::{String, ToString};
use core::arch::asm;

use acpi::InterruptModel::*;
use x86_64::{
    structures::paging::{PageTableFlags, PhysFrame},
    PhysAddr,
};

use crate::{debug, memory::KERNEL_MEMORY_MANAGER};

use super::{acpi::get_acpi_tables, timer::SPIN_TIMER};

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
const APIC_REGISTER_OFFSET_ERROR_STATUS: usize = 0x280;
const APIC_REGISTER_IPI_LOW: usize = 0x300;
const APIC_REGISTER_IPI_HIGH: usize = 0x310;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct AdvancedProgrammableInterruptController {
    address: *mut u8,
}

impl AdvancedProgrammableInterruptController {
    fn read_register(self, register: usize) -> u32 {
        let pointer = self.get_register_pointer(register);
        unsafe { pointer.read_volatile() }
    }

    #[inline]
    fn get_register_pointer(self, register: usize) -> *mut u32 {
        let register_address = (register & APIC_REGISTER_ADDRESS_MASK) as isize;
        unsafe { self.address.byte_offset(register_address as isize) as *mut u32 }
    }

    #[inline]
    fn write_register(self, register: usize, value: u32) {
        let pointer = self.get_register_pointer(register);
        let pointer = core::hint::black_box(pointer);
        unsafe {
            asm!("sfence");
            pointer.write_volatile(value);
            asm!("sfence");
        }
    }

    #[inline]
    pub fn get_apic_id(self) -> u32 {
        self.read_register(APIC_REGISTER_OFFSET_ID)
    }

    #[inline]
    pub fn set_apic_id(self, value: u32) {
        self.write_register(APIC_REGISTER_OFFSET_ID, value);
    }

    #[inline]
    pub fn get_version(self) -> u32 {
        self.read_register(APIC_REGISTER_OFFSET_VERSION)
    }

    #[inline]
    pub fn get_task_priority(self) -> u32 {
        self.read_register(APIC_REGISTER_OFFSET_TASK_PRIORITY)
    }

    #[inline]
    pub fn get_arbiration_priority(self) -> u32 {
        self.read_register(APIC_REGISTER_OFFSET_ARBITRATION_PRIORITY)
    }

    #[inline]
    pub fn get_processor_priority(self) -> u32 {
        self.read_register(APIC_REGISTER_OFFSET_PROCESSOR_PRIORITY)
    }

    #[inline]
    pub fn get_spurious_interrupt_vector(self) -> u32 {
        self.read_register(APIC_REGISTER_OFFSET_SPURIOUS_INTERRUPT_VECTOR)
    }

    #[inline]
    pub fn set_spurious_interrupt_vector(self, value: u32) {
        self.write_register(APIC_REGISTER_OFFSET_SPURIOUS_INTERRUPT_VECTOR, value)
    }

    #[inline]
    pub fn get_local_vector_table_timer(self) -> u32 {
        self.read_register(APIC_REGISTER_OFFSET_LOCAL_VECTOR_TABLE_TIMER)
    }

    #[inline]
    pub fn set_local_vector_table_timer(self, value: u32) {
        self.write_register(APIC_REGISTER_OFFSET_LOCAL_VECTOR_TABLE_TIMER, value);
    }

    #[inline]
    pub fn get_timer_divisor(self) -> u32 {
        self.read_register(APIC_REGISTER_OFFSET_TIMER_DIVISOR)
    }

    #[inline]
    pub fn set_timer_divisor(self, value: u32) {
        self.write_register(APIC_REGISTER_OFFSET_TIMER_DIVISOR, value);
    }

    #[inline]
    pub fn set_timer_initial_count(self, value: u32) {
        self.write_register(APIC_REGISTER_OFFSET_TIMER_INITIAL_COUNT, value);
    }

    #[inline]
    pub fn end_of_interrupt(self) {
        self.write_register(APIC_REGISTER_OFFSET_END_OF_INTERRUPT, 0);
    }

    #[inline]
    pub fn get_error_status(self) -> u32 {
        self.read_register(APIC_REGISTER_OFFSET_ERROR_STATUS)
    }

    #[inline]
    pub fn set_error_status(self, value: u32) {
        self.write_register(APIC_REGISTER_OFFSET_ERROR_STATUS, value)
    }

    #[inline]
    pub fn get_ipi_high(self) -> u32 {
        self.read_register(APIC_REGISTER_IPI_HIGH)
    }

    #[inline]
    pub fn get_ipi_low(self) -> u32 {
        self.read_register(APIC_REGISTER_IPI_LOW)
    }

    #[inline]
    pub fn get_ipi(self) -> u64 {
        let ipi_high = self.get_ipi_high() as u64;
        let ipi_low = self.get_ipi_low() as u64;

        (ipi_high << 32) | (ipi_low & u32::MAX as u64)
    }

    #[inline]
    pub fn wait_for_ipi_delivery(self) {
        const PENDING: u32 = 1 << 12;
        while self.get_ipi_high() & PENDING == PENDING {
            core::hint::spin_loop();
        }
    }

    #[inline]
    pub fn send_ipi_init(self, cpu_id: u64) {
        self.clear_apic_errors();
        // Assert INIT
        let icr_value = cpu_id << 56 | 0x4500;
        self.send_ipi(icr_value);
        self.wait_for_ipi_delivery();
    }

    #[inline]
    pub fn send_ipi_start(self, cpu_id: u64, segment: u8) {
        self.clear_apic_errors();
        // SIPI
        let icr_value = cpu_id << 56 | 0x4600 | (segment as u64);
        self.send_ipi(icr_value);
        self.wait_for_ipi_delivery();
    }

    #[inline]
    pub fn send_ipi_32(self, ipi_high: u32, ipi_low: u32) {
        debug!("IPI: {:08x} {:08x}", ipi_high, ipi_low);
        self.set_ipi_high(ipi_high);
        self.set_ipi_low(ipi_low);
        self.wait_for_ipi_delivery();
    }

    pub fn clear_apic_errors(self) {
        self.write_register(APIC_REGISTER_OFFSET_ERROR_STATUS, 0);
    }

    #[inline]
    pub fn send_ipi(self, value: u64) {
        let ipi_high = (value >> 32) & u32::MAX as u64;
        let ipi_high = ipi_high as u32;
        let ipi_low = value as u32;
        self.send_ipi_32(ipi_high, ipi_low);
    }

    #[inline]
    pub fn set_ipi_high(self, value: u32) {
        self.write_register(APIC_REGISTER_IPI_HIGH, value);
    }

    #[inline]
    pub fn set_ipi_low(self, value: u32) {
        self.write_register(APIC_REGISTER_IPI_LOW, value);
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
    KERNEL_MEMORY_MANAGER.lock().identity_map(
        PhysFrame::containing_address(PhysAddr::new_truncate(addr)),
        PageTableFlags::WRITABLE | PageTableFlags::NO_CACHE | PageTableFlags::PRESENT,
    );
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
        debug!("APIC setup complete.");
    }
}
