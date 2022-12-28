use core::mem;

pub const ARCH_WORD_SIZE: usize = mem::size_of::<usize>();
pub const NULL_POINTER: *mut u8 = 0 as *mut u8;

#[derive(Debug, Clone, Copy)]
#[repr(usize)]
pub enum SyscallNumber {
    Invalid,
    ContextSwitch,
    AllocatePage,
    AllocatePageRange
}