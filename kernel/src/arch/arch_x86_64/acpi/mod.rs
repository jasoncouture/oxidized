use core::{cell::OnceCell, panic, ptr::NonNull};

use acpi::{madt::MadtEntry, AcpiHandler, AcpiTables, InterruptModel};
use alloc::{boxed::Box, vec::Vec};
use x86_64::{PhysAddr, VirtAddr};

use crate::{
    debug,
    memory::{self, allocator::KERNEL_FRAME_ALLOCATOR, KERNEL_MEMORY_MANAGER},
    warn,
};

#[derive(Clone, Copy)]
pub struct AcpiHandlerImpl {}

impl AcpiHandler for AcpiHandlerImpl {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> acpi::PhysicalMapping<Self, T> {
        let memory_manager = KERNEL_MEMORY_MANAGER.lock();
        let mapped_address = memory_manager.translate(PhysAddr::new(physical_address as u64));

        let val = mapped_address.as_ptr() as *const T;
        let nonnull_val = NonNull::from(val.as_ref().unwrap());
        acpi::PhysicalMapping::new(physical_address, nonnull_val, size, size, *self)
    }

    fn unmap_physical_region<T>(_region: &acpi::PhysicalMapping<Self, T>) {
        // Not needed.
    }
}

static ACPI_HANDLER: AcpiHandlerImpl = AcpiHandlerImpl {};
static mut ACPI_TABLES: OnceCell<AcpiTables<AcpiHandlerImpl>> = OnceCell::new();

unsafe fn load_acpi(rsdp_addr: Option<u64>) -> AcpiTables<AcpiHandlerImpl> {
    match rsdp_addr {
        Some(addr) => acpi::AcpiTables::from_rsdp(ACPI_HANDLER, addr as usize),
        None => acpi::AcpiTables::search_for_rsdp_bios(ACPI_HANDLER),
    }
    .expect("Unable to find ACPI tables!")
}

pub(crate) fn get_acpi_tables() -> &'static mut AcpiTables<AcpiHandlerImpl> {
    unsafe {
        match ACPI_TABLES.get_mut() {
            Some(res) => res,
            None => panic!("Attempted to get ACPI tables before initialization"),
        }
    }
}

pub(crate) fn init(rsdp_addr: Option<u64>) {
    unsafe {
        if ACPI_TABLES.get().is_none() {
            if ACPI_TABLES.set(load_acpi(rsdp_addr)).is_err() {
                panic!("Failed to set ACPI tables after parsing, this should never happen!");
            }
        } else {
            warn!("Attempted to re-initialize ACPI tables. Ignoring.");
            return;
        }
        let acpi_tables = ACPI_TABLES.get_mut().unwrap();

        debug!("Loaded ACPI Tables, Revison: {}", acpi_tables.revision);
        let platform_info = acpi_tables
            .platform_info()
            .expect("Unable to retrieve platform info from ACPI!");

        //debug!("Interrupt model: {:?}", platform_info.interrupt_model);
        let cpu_info = platform_info
            .processor_info
            .expect("Unable to read processor configuration!");

        debug!("Processor info:");
        debug!("-- {:?}", cpu_info.boot_processor);
        for processor in cpu_info.application_processors {
            debug!("-- {:?}", processor);
        }
    };
}
