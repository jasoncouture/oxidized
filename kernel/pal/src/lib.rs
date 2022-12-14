#![no_std]

pub trait HardwareControl {
    fn init(&self);
    fn halt(&self) -> !;
}

pub trait Driver {
    fn name(&self) -> &str;
}