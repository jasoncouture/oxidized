#![no_std]

use constants::ARCH_WORD_SIZE;

pub mod constants;
pub mod handle;
pub mod ipc;
pub mod memory;
pub mod syscall;