use core::{str::FromStr, fmt::Display};

use alloc::string::String;


#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Ord, Eq)]
pub enum SyscallErrorCode {
    None = 0,
    NoSyscall = 255
}

#[derive(Clone, Debug)]
pub struct SyscallError { error_code: SyscallErrorCode, message: String }

impl SyscallError {
    pub fn new(error_code: SyscallErrorCode, message: String) -> Self {
        Self {
            error_code,
            message
        }
    }

    pub fn no_such_system_call() -> Self {
        Self::new(SyscallErrorCode::NoSyscall, String::from_str("No such system call").unwrap())
    }
}

impl Display for SyscallError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:016x} - {:?} - {}", self.error_code as u64, self.error_code, self.message)
    }
}

impl core::error::Error for SyscallError { }