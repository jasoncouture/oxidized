use core::mem;

pub const ARCH_WORD_SIZE: usize = mem::size_of::<usize>();

#[derive(Debug, Clone, Copy)]
#[repr(usize)]
pub enum SyscallNumber {
    Invalid,
    ContextSwitch,
    AllocatePage,
    AllocatePageRange
}