use alloc::vec::Vec;
use core::cell::OnceCell;
use spin::Mutex;

#[repr(align(16))]
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ProcessDescriptor {
    id: u64,
    control_group: u64,
}

impl ProcessDescriptor {
    pub fn new(id: u64) -> Self {
        Self {
            control_group: 0,
            id,
        }
    }

    // PID of this process
    pub fn get_id(&self) -> u64 {
        self.id
    }
    // control group, reserved, should always be 0
    pub fn get_control_group(&self) -> u64 {
        self.control_group
    }
}

pub struct ProcessManager {
    processes: Mutex<Vec<ProcessDescriptor>>,
    next_process_id: u64,
}

impl ProcessManager {
    pub fn new() -> Self {
        let mut vec = Vec::new();
        vec.reserve(64);
        Self {
            processes: Mutex::new(vec),
            next_process_id: 0,
        }
    }

    pub fn get_process(&self, id: u64) -> Option<ProcessDescriptor> {
        let locked_processes = self.processes.lock();
        let index = locked_processes.binary_search_by_key(&id, |f| f.id).ok()?;
        locked_processes.get(index).copied()
    }

    pub fn create_process(&mut self) -> ProcessDescriptor {
        // We intentionally do not use get_process here, because we need to hold the lock the entire time.
        let locked_processes = self.processes.get_mut();
        let current = self.next_process_id;
        loop {
            // this is for when we wrap.
            // Processes can come and go, but anti-collision code is forever.
            let insert_index = locked_processes
                .binary_search_by_key(&current, |p| p.id)
                .err();

            if insert_index.is_none() {
                continue;
            }

            self.next_process_id = current.wrapping_add(1);
            let descriptor = ProcessDescriptor::new(current);
            locked_processes.insert(insert_index.unwrap(), descriptor);
            return descriptor;
        }
    }
}

static mut PROCESS_MANAGER: OnceCell<ProcessManager> = OnceCell::new();

pub fn process_manager() -> &'static mut ProcessManager {
    unsafe {
        match PROCESS_MANAGER.get() {
            None => {
                PROCESS_MANAGER.get_or_init(ProcessManager::new);
                PROCESS_MANAGER.get_mut().unwrap()
            }
            Some(_) => unsafe { PROCESS_MANAGER.get_mut().unwrap() },
        }
    }
}
