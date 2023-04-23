#![no_std]

use alloc::collections::BTreeMap;
extern crate alloc;


pub enum DeviceState {
    Unknown,
    NoService,
    Locked,
    Disabled,
    Operational
}

#[repr(C, align(1))]
#[derive(Clone, Copy, Debug)]
pub struct DeviceQueryResult{
    pub id: u64,
    pub class_id: u64,
    pub subclass_id: u64,
    pub device_type_id: u64,
    pub service_id: u64
}

pub struct DeviceRegistry {
    devices: BTreeMap<u64, DeviceQueryResult>   
}

impl DeviceRegistry {
    
}


pub trait SystemService {
    fn control(&mut self, id: u64, command: u64, data: *const u8) -> *const u8;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
