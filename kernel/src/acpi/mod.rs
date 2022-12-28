use core::ptr::NonNull;

use acpi::{AcpiHandler, AcpiTables};
use alloc::{boxed::Box, vec::Vec};
use x86_64::{PhysAddr, VirtAddr};

use crate::{memory::{self, allocator::KERNEL_FRAME_ALLOCATOR, KERNEL_MEMORY_MANAGER}, debug};

#[derive(Clone, Copy)]
struct AcpiHandlerImpl {}

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

    fn unmap_physical_region<T>(region: &acpi::PhysicalMapping<Self, T>) {
        // Not needed.
    }
}

static ACPI_HANDLER: AcpiHandlerImpl = AcpiHandlerImpl {};

pub(crate) fn init(rsdp_addr: Option<u64>) {
    let addr = rsdp_addr.unwrap();

    unsafe {
        let acpi_tables: AcpiTables<AcpiHandlerImpl> = acpi::AcpiTables::from_rsdp(ACPI_HANDLER, addr as usize).expect("Unable to parse ACPI tables!");
        debug!("Loaded ACPI Tables, Revison: {}", acpi_tables.revision);
        let platform_info = acpi_tables.platform_info().expect("Unable to retrieve platform info from ACPI!");
        //debug!("Interrupt model: {:?}", platform_info.interrupt_model);
        let cpu_info = platform_info.processor_info.expect("Unable to read processor configuration!");
        debug!("Processor info:");
        debug!("-- {:?}", cpu_info.boot_processor);
        for processor in cpu_info.application_processors {
            debug!("-- {:?}", processor);
        }
    };
}
