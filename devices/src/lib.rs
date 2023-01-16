#![no_std]
#![feature(once_cell)]
#![feature(core_intrinsics)]
#![feature(error_in_core)]
extern crate alloc;

pub mod well_known;

use core::{
    cell::OnceCell, error::Error, fmt::Display, intrinsics::type_name,
};

use alloc::{
    boxed::Box,
    collections::BTreeMap,
    string::{String, ToString},
    vec::Vec,
};
use spin::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Handle {
    device_id: u128,
    device_resource_id: u128,
}

pub struct DeviceTreeDevice {}

impl Device for DeviceTreeDevice{
    fn name(&self) -> String {
        "DEVICE_TREE".to_string()
    }

    fn ready(&self) -> bool {
        true
    }

    fn function(&self, id: usize, args: &[usize]) -> Result<&[u8], DeviceError> {
        match id {
            0 => {
                todo!()
            },
            1 => {
                todo!()
            }
            _ => Err(DeviceError::new(DeviceErrorCode::NotImplemented))
        }
    }

    fn uuid(&self) -> Uuid {
        *well_known::DEVICE_TREE
    }
}

#[cfg(feature = "kernel")]
pub struct DeviceTree {
    map: BTreeMap<u128, Box<dyn Device>>,
}

#[cfg(feature = "kernel")]
impl DeviceTree {
    fn new() -> Self {
        let mut ret = Self {
            map: BTreeMap::new(),
        };
        ret.register(DeviceTreeDevice{});
        ret
    }

    pub fn register(&mut self, device: impl Device + 'static) -> u128 {
        let mut current = device.uuid().as_u128();
        while self.map.contains_key(&current) {
            current = current.wrapping_add(1);
        }

        self.map.insert(current, Box::new(device));
        current
    }

    pub fn get_device_path(&self, device: &(impl Device + ?Sized)) -> String {
        let mut ret = String::new();
        ret.insert_str(0, device.name().as_str());
        let mut next = match device.parent_id() {
            Some(p) => self.get(&p),
            None => None,
        };

        while next.is_some() {
            let current = next.unwrap();
            next = match current.parent_id() {
                Some(p) => match self.get(&p) {
                    Some(s) => Some(s),
                    None => panic!("Device {} contains reference to parent id {:032x}, but that parent is not in the device tree!", current.uuid().as_hyphenated(), p)
                }
                None => None
            };

            ret.insert(0, '/');
            ret.insert_str(0, current.name().as_str());
        }

        ret
    }

    pub fn unregister(&mut self, id: u128) -> Option<Box<dyn Device>> {
        self.map.remove(&id)
    }

    pub fn get(&self, id: &u128) -> Option<&dyn Device> {
        match self.map.get(id) {
            Some(v) => Some(v.as_ref()),
            None => None,
        }
    }

    pub fn get_mut(&mut self, id: &u128) -> Option<&mut dyn Device> {
        match self.map.get_mut(id) {
            Some(v) => Some(v.as_mut()),
            None => None,
        }
    }

    pub fn keys(&self) -> Vec<u128> {
        let mut v = Vec::new();
        for k in self.map.iter() {
            v.push(*k.0);
        }
        v
    }

    pub fn all(&self) -> Vec<&dyn Device> {
        let mut ret = Vec::new();
        for kv in self.map.iter() {
            ret.push(kv.1.as_ref());
        }
        ret
    }
}

#[cfg(feature = "kernel")]
pub fn get_mut_device_tree() -> RwLockWriteGuard<'static, DeviceTree> {
    unsafe {
        DEVICE_TREE
            .get_or_init(|| RwLock::new(DeviceTree::new()))
            .write()
    }
}

#[cfg(feature = "kernel")]
pub fn get_device_tree() -> RwLockReadGuard<'static, DeviceTree> {
    unsafe {
        DEVICE_TREE
            .get_or_init(|| RwLock::new(DeviceTree::new()))
            .read()
    }
}

#[cfg(feature = "kernel")]
static mut DEVICE_TREE: OnceCell<RwLock<DeviceTree>> = OnceCell::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceErrorCode {
    NotImplemented,
    Malfunction,
    DeviceNativeError(u64),
}

#[derive(Debug, Clone, Copy)]
pub struct DeviceError {
    error_code: DeviceErrorCode,
}

impl DeviceError {
    pub fn new(error_code: DeviceErrorCode) -> Self {
        DeviceError { error_code }
    }
}

impl Display for DeviceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for DeviceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }

    fn description(&self) -> &str {
        "description() is deprecated; use Display"
    }

    fn cause(&self) -> Option<&dyn Error> {
        self.source()
    }
}

pub trait Device: Sync + Send {
    fn uuid(&self) -> Uuid {
        Uuid::nil()
    }
    fn parent_id(&self) -> Option<u128> {
        None
    }
    fn name(&self) -> String {
        type_name::<Self>().to_string()
    }
    fn ready(&self) -> bool;

    #[allow(unused_variables)]
    fn function(&self, id: usize, args: &[usize]) -> Result<&[u8], DeviceError> {
        Err(DeviceError::new(DeviceErrorCode::NotImplemented))
    }
}
