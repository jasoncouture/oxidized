use core::arch::asm;

use alloc::{collections::BTreeMap};
use lazy_static::lazy_static;
use spin::RwLock;


use crate::{debug, errors::SyscallError};

pub fn init() {
    // // IA32_STAR[31:0] are reserved.
    // // The base selector of the two consecutive segments for kernel code and the immediately
    // // suceeding stack (data).
    // let syscall_cs_ss_base = BOOT_GDT.get_kernel_code_segment().0;
    // // The base selector of the three consecutive segments (of which two are used) for user code
    // // and user data. It points to a 32-bit code segment, which must be followed by a data segment
    // // (stack), and a 64-bit code segment.
    // let sysret_cs_ss_base = BOOT_GDT.get_user_code_segment().0 | 3;
    // let star_high = u32::from(syscall_cs_ss_base) | (u32::from(sysret_cs_ss_base) << 16);
    // unsafe {
    //     msr::wrmsr(msr::IA32_STAR, u64::from(star_high) << 32);
    //     msr::wrmsr(msr::IA32_LSTAR, syscall_instruction as u64);
    //     msr::wrmsr(msr::IA32_FMASK, 0x0300); // Clear trap flag and interrupt enable

    //     let efer = msr::rdmsr(msr::IA32_EFER);
    //     msr::wrmsr(msr::IA32_EFER, efer | 1);
    // }
    let mut native_personality = SyscallTable::new();
    native_personality.set_default_handler(native_default_syscall_handler);
    SYSCALL_TABLES
        .write()
        .register_personality(usize::MAX, native_personality);
}

fn native_default_syscall_handler(parameters: &SyscallParameters) {
    debug!("Unknown syscall: {}", parameters.id);
}

pub struct SyscallParameters {
    id: usize,
}

impl SyscallParameters {
    pub fn new(id: usize) -> Self {
        Self { id }
    }
}

type SyscallEntry = fn(&SyscallParameters);
#[derive(Clone)]
pub struct SyscallTable {
    calls: BTreeMap<usize, SyscallEntry>,
}

impl SyscallTable {
    pub fn new() -> Self {
        SyscallTable {
            calls: BTreeMap::new(),
        }
    }
    pub fn try_get_syscall(
        &self,
        parameters: &SyscallParameters,
    ) -> Result<SyscallEntry, SyscallError> {
        if let Some(entry) = self.calls.get(&parameters.id) {
            Ok(*entry)
        } else if let Some(entry) = self.calls.get(&usize::MAX) {
            Ok(*entry)
        } else {
            Err(SyscallError::no_such_system_call())
        }
    }

    pub fn set_default_handler(&mut self, callback: SyscallEntry) {
        self.set_handler(usize::MAX, callback);
    }

    pub fn set_handler(&mut self, id: usize, callback: SyscallEntry) {
        self.calls.insert(id, callback);
    }
}

pub struct SyscallTables {
    tables: BTreeMap<usize, SyscallTable>,
}

impl SyscallTables {
    pub fn register_personality(&mut self, id: usize, personality_table: SyscallTable) {
        self.tables.insert(id, personality_table);
    }

    pub fn get_personality(&self, id: usize) -> Option<SyscallTable> {
        let result = self.tables.get(&id)?;
        Some(result.clone())
    }

    pub fn update_personality(&mut self, id: usize, callback: fn(&mut SyscallTable)) {
        if let Some(mut table) = self.get_personality(id) {
            callback(&mut table);
            self.register_personality(id, table);
        } else {
            let mut table = SyscallTable::new();
            callback(&mut table);
            self.register_personality(id, table); 
        }
    }
}

lazy_static! {
    pub static ref SYSCALL_TABLES: RwLock<SyscallTables> = RwLock::new(SyscallTables {
        tables: BTreeMap::new()
    });
}

pub unsafe extern "x86-interrupt" fn syscall_instruction() {
    asm!("sysretq", options(noreturn));
    unreachable!();
}
