use core::panic;

use acpi::InterruptModel::*;

use x86::msr::{
    rdmsr, wrmsr, IA32_APIC_BASE, IA32_X2APIC_APICID, IA32_X2APIC_DIV_CONF, IA32_X2APIC_EOI,
    IA32_X2APIC_ICR, IA32_X2APIC_INIT_COUNT, IA32_X2APIC_LVT_ERROR, IA32_X2APIC_LVT_TIMER,
    IA32_X2APIC_PPR, IA32_X2APIC_SIVR, IA32_X2APIC_TPR, IA32_X2APIC_VERSION,
};
use x86_64::{
    structures::paging::{PageTableFlags, PhysFrame},
    PhysAddr,
};

use crate::{debug, memory::KERNEL_MEMORY_MANAGER};

use super::{acpi::get_acpi_tables, cpuid::cpuid};

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
const APIC_REGISTER_OFFSET_LOCAL_VECTOR_TABLE_ERROR: usize = 0x370;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct AdvancedProgrammableInterruptController {
    address: *mut u8,
    x2: bool,
}

impl AdvancedProgrammableInterruptController {
    fn read_register(&self, register: usize) -> u32 {
        let pointer = self.get_register_pointer(register);
        unsafe { pointer.read_volatile() }
    }

    #[inline]
    fn get_register_pointer(&self, register: usize) -> *mut u32 {
        if self.x2 {
            panic!("Attempted to use local xAPIC address, when using x2 APIC!");
        }
        let register_address = (register & APIC_REGISTER_ADDRESS_MASK) as isize;
        unsafe { self.address.byte_offset(register_address as isize) as *mut u32 }
    }

    #[inline]
    fn write_register(&self, register: usize, value: u32) {
        if self.x2 {
            panic!("Attempted to use local xAPIC address, when using x2 APIC!");
        }
        let pointer = self.get_register_pointer(register);
        unsafe {
            pointer.write_volatile(value);
        }
    }

    pub fn read_apic_msr(&self, msr: u32) -> u64 {
        if !self.x2 {
            panic!("Attempted to use X2 APIC, but X2 APIC was not detected!");
        }
        unsafe { rdmsr(msr) }
    }

    pub fn write_apic_msr(&self, msr: u32, value: u64) {
        if !self.x2 {
            panic!("Attempted to use X2 APIC, but X2 APIC was not detected!");
        }
        unsafe { wrmsr(msr, value) }
    }

    #[inline]
    pub fn get_apic_id(&self) -> u64 {
        if !self.x2 && self.address as usize == 0 {
            return 0;
        }
        if self.x2 {
            self.read_apic_msr(IA32_X2APIC_APICID) & u32::MAX as u64
        } else {
            self.read_register(APIC_REGISTER_OFFSET_ID) as u64
        }
    }

    #[inline]
    pub fn get_version(&self) -> u64 {
        if self.x2 {
            self.read_apic_msr(IA32_X2APIC_VERSION)
        } else {
            self.read_register(APIC_REGISTER_OFFSET_VERSION) as u64
        }
    }

    #[inline]
    pub fn get_task_priority(&self) -> u64 {
        if self.x2 {
            self.read_apic_msr(IA32_X2APIC_TPR)
        } else {
            self.read_register(APIC_REGISTER_OFFSET_TASK_PRIORITY) as u64
        }
    }

    #[inline]
    pub fn get_arbiration_priority(&self) -> u64 {
        if self.x2 {
            self.read_apic_msr(IA32_X2APIC_PPR)
        } else {
            self.read_register(APIC_REGISTER_OFFSET_ARBITRATION_PRIORITY) as u64
        }
    }

    #[inline]
    pub fn get_processor_priority(&self) -> u64 {
        if self.x2 {
            self.read_apic_msr(IA32_X2APIC_PPR)
        } else {
            self.read_register(APIC_REGISTER_OFFSET_PROCESSOR_PRIORITY) as u64
        }
    }

    #[inline]
    pub fn get_spurious_interrupt_vector(&self) -> u64 {
        if self.x2 {
            self.read_apic_msr(IA32_X2APIC_SIVR)
        } else {
            self.read_register(APIC_REGISTER_OFFSET_SPURIOUS_INTERRUPT_VECTOR) as u64
        }
    }

    #[inline]
    pub fn set_spurious_interrupt_vector(&self, value: u64) {
        if self.x2 {
            self.write_apic_msr(IA32_X2APIC_SIVR, value)
        } else {
            self.write_register(APIC_REGISTER_OFFSET_SPURIOUS_INTERRUPT_VECTOR, value as u32)
        }
    }

    #[inline]
    pub fn get_timer_divisor(&self) -> u64 {
        if self.x2 {
            self.read_apic_msr(IA32_X2APIC_DIV_CONF)
        } else {
            self.read_register(APIC_REGISTER_OFFSET_TIMER_DIVISOR) as u64
        }
    }

    #[inline]
    pub fn set_timer_divisor(&self, value: u32) {
        if self.x2 {
            self.write_apic_msr(IA32_X2APIC_DIV_CONF, value as u64);
        } else {
            self.write_register(APIC_REGISTER_OFFSET_TIMER_DIVISOR, value);
        }
    }

    #[inline]
    pub fn set_timer_initial_count(&self, value: u32) {
        if self.x2 {
            self.write_apic_msr(IA32_X2APIC_INIT_COUNT, value as u64);
        } else {
            self.write_register(APIC_REGISTER_OFFSET_TIMER_INITIAL_COUNT, value);
        }
    }

    #[inline]
    pub fn end_of_interrupt(&self) {
        if self.x2 {
            self.write_apic_msr(IA32_X2APIC_EOI, 0);
        } else {
            self.write_register(APIC_REGISTER_OFFSET_END_OF_INTERRUPT, 0);
        }
    }

    #[inline]
    pub fn get_error_status(&self) -> u64 {
        if self.x2 {
            self.write_apic_msr(IA32_X2APIC_LVT_ERROR, 0);
            self.read_apic_msr(IA32_X2APIC_LVT_ERROR)
        } else {
            self.write_register(APIC_REGISTER_OFFSET_ERROR_STATUS, 0);
            self.read_register(APIC_REGISTER_OFFSET_ERROR_STATUS) as u64
        }
    }

    #[inline]
    pub fn set_error_status(&self, value: u64) {
        if self.x2 {
            self.write_apic_msr(IA32_X2APIC_LVT_ERROR, value)
        } else {
            self.write_register(APIC_REGISTER_OFFSET_ERROR_STATUS, value as u32)
        }
    }

    #[inline]
    pub fn get_icr(&self) -> u64 {
        if self.x2 {
            self.read_apic_msr(IA32_X2APIC_ICR)
        } else {
            let ipi_high = self.read_register(APIC_REGISTER_IPI_HIGH) as u64;
            let ipi_low = self.read_register(APIC_REGISTER_IPI_LOW) as u64;

            (ipi_high << 32) | (ipi_low & u32::MAX as u64)
        }
    }

    #[inline]
    pub fn wait_for_ipi_delivery(&self) {
        if self.x2 {
            return;
        }
        const PENDING: u64 = 1 << 12;
        while self.get_icr() & PENDING == PENDING {
            core::hint::spin_loop();
        }
    }

    #[inline]
    pub fn send_ipi_init(&self, cpu_id: usize) {
        self.clear_apic_errors();
        // Assert INIT
        let icr_value: u64 = 0x4500 | self.get_icr_cpu_value(cpu_id);
        self.set_icr(icr_value);
    }

    fn get_icr_cpu_value(&self, cpu_id: usize) -> u64 {
        let shift = match self.x2 {
            true => 32,
            false => 56,
        };
        (cpu_id as u64) << shift
    }

    #[inline]
    pub fn send_ipi_start(&self, cpu_id: usize, segment: u8) {
        self.clear_apic_errors();
        // SIPI
        let icr_value = self.get_icr_cpu_value(cpu_id) | 0x4600 | (segment as u64);
        self.set_icr(icr_value);
    }

    pub fn clear_apic_errors(&self) {
        if self.x2 {
            self.write_apic_msr(IA32_X2APIC_LVT_ERROR, 0);
        } else {
            self.write_register(APIC_REGISTER_OFFSET_ERROR_STATUS, 0);
        }
    }

    #[inline]
    pub fn set_icr(&self, value: u64) {
        self.wait_for_ipi_delivery();
        if self.x2 {
            self.write_apic_msr(IA32_X2APIC_ICR, value)
        } else {
            let ipi_high = (value >> 32) & u32::MAX as u64;
            let ipi_high = ipi_high as u32;
            self.wait_for_ipi_delivery();
            self.write_register(APIC_REGISTER_IPI_HIGH, ipi_high);
            let ipi_low = value as u32;

            self.write_register(APIC_REGISTER_IPI_LOW, ipi_low);
            self.wait_for_ipi_delivery();
        }
    }

    pub fn set_local_vector_table_timer(&self, value: u64) {
        if self.x2 {
            self.write_apic_msr(IA32_X2APIC_LVT_TIMER, value)
        } else {
            self.write_register(APIC_REGISTER_OFFSET_LOCAL_VECTOR_TABLE_TIMER, value as u32);
        }
    }
}

pub(crate) static mut LOCAL_APIC: AdvancedProgrammableInterruptController =
    AdvancedProgrammableInterruptController {
        address: 0 as *mut u8,
        x2: false,
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

    let x2_apic = cpuid().map_or(false, |r| {
        r.get_feature_info()
            .map_or(false, |feature| feature.has_x2apic())
    });
    if x2_apic {
        unsafe {
            LOCAL_APIC.x2 = true;
        }
        debug!("System has x2 apic support, using that instead of legacy APIC");
    } else {
        debug!("Local APIC address: {:p}", addr as usize as *const ());
        KERNEL_MEMORY_MANAGER.lock().identity_map(
            PhysFrame::containing_address(PhysAddr::new_truncate(addr)),
            PageTableFlags::WRITABLE | PageTableFlags::NO_CACHE | PageTableFlags::PRESENT,
        );
        let apic_ptr: *mut u8 = addr as *mut u8;
        unsafe {
            LOCAL_APIC.address = apic_ptr;
        }
    }

    unsafe {
        init_ap();
    }
}

pub(crate) unsafe fn init_ap() {
    if LOCAL_APIC.x2 {
        LOCAL_APIC.write_apic_msr(
            IA32_APIC_BASE,
            LOCAL_APIC.read_apic_msr(IA32_APIC_BASE) | 1 << 10,
        );
    }
    let mut sivr = LOCAL_APIC.get_spurious_interrupt_vector();
    sivr = sivr | 0x1FF;
    LOCAL_APIC.set_spurious_interrupt_vector(sivr);
    debug!("Starting timer on IRQ0 (Vector 32)");
    // 0x20000 - Enable periodic timer, 32 == interrupt vector (IRQ0, Vector 32)
    LOCAL_APIC.set_local_vector_table_timer(32 | 0x20000);
    LOCAL_APIC.set_timer_divisor(0x03);
    LOCAL_APIC.set_timer_initial_count(0xFF00);
    debug!("APIC setup complete.");
}
