#![no_std]
#![feature(core_intrinsics)]
#![feature(trait_upcasting)]
extern crate alloc;
extern crate core;

pub mod constants;
pub mod handle;
pub mod ipc;
pub mod kernel_state;
pub mod memory;
pub mod syscall;
