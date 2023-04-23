use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use core::any::Any;
use core::fmt::Debug;
use core::intrinsics::type_name;
use uuid::Uuid;

#[derive(Copy, Clone, Debug)]
struct BootData {}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Ord, Eq)]
#[repr(u64)]
pub enum DeviceClass {
    Unknown = 0,
    Kernel = 1,
}

#[derive(Clone)]
pub struct DeviceDescriptor {
    device: &'static dyn Any,
    device_id: u128,
    device_parent_id: Option<u128>,
    device_class: DeviceClass,
}

impl DeviceDescriptor {
    pub fn new<T>(device: &'static T, device_class: DeviceClass) -> Self
    where
        T: Device<T>,
    {
        Self {
            device: device,
            device_id: device.uuid().as_u128(),
            device_parent_id: device.parent_id(),
            device_class: device_class,
        }
    }
}

pub trait DeviceBase: 'static + Clone + Copy + Any {
    fn try_into<T>(&self) -> Option<&'static T>;
    fn try_into_mut<T>(&self) -> Option<&'static mut T>;
}

pub trait Device<T>: 'static + DeviceBase {
    fn uuid(&self) -> Uuid {
        Uuid::nil()
    }
    fn parent_id(&self) -> Option<u128> {
        None
    }
    fn name(&self) -> String {
        type_name::<Self>().to_string()
    }
    fn ready(&self) -> bool {
        true
    }

    fn inner(&self) -> &T;
}

pub trait VirtualMemoryManager {
    fn heap_alloc(&mut self, size: usize, zero: bool) -> anyhow::Result<*mut u8>;
    fn heap_free(&mut self, size: usize);
    fn page_alloc(&mut self, count: usize, zero: bool) -> anyhow::Result<*mut u8>;
    fn page_free(&mut self, pages_start: *mut u8, count: usize);
    fn physical_page_alloc(
        &mut self,
        physical_address: usize,
        zero: bool,
    ) -> anyhow::Result<*mut u8>;
}

struct KernelState {
    boot_info: BootData,
    devices: BTreeMap<u128, DeviceDescriptor>,
    memory_manager: &'static mut dyn VirtualMemoryManager,
}

impl VirtualMemoryManager for KernelState {
    fn heap_alloc(&mut self, size: usize, zero: bool) -> anyhow::Result<*mut u8> {
        self.memory_manager.heap_alloc(size, zero)
    }

    fn heap_free(&mut self, size: usize) {
        self.memory_manager.heap_free(size)
    }

    fn page_alloc(&mut self, count: usize, zero: bool) -> anyhow::Result<*mut u8> {
        self.memory_manager.page_alloc(count, zero)
    }

    fn page_free(&mut self, pages_start: *mut u8, count: usize) {
        self.memory_manager.page_free(pages_start, count)
    }

    fn physical_page_alloc(
        &mut self,
        physical_address: usize,
        zero: bool,
    ) -> anyhow::Result<*mut u8> {
        self.memory_manager
            .physical_page_alloc(physical_address, zero)
    }
}

impl KernelState {
    pub fn new<T>(
        boot_info: BootData,
        memory_manager: &'static mut dyn VirtualMemoryManager,
    ) -> Self
    where
        T: VirtualMemoryManager,
    {
        KernelState {
            boot_info,
            devices: BTreeMap::new(),
            memory_manager,
        }
    }
}
