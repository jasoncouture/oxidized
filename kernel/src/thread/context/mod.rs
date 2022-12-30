use core::{cmp::max, ops::Index, ptr::NonNull};

use alloc::{boxed::Box, vec::Vec};
use lazy_static::*;
use spin::{Mutex, RwLock};

// The context struct is used to track and maintain the state of a thread.
#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum ContextState {
    Initializing,
    Ready,
    Running,
    Blocked,
    Terminating,
    Dead,
}

#[repr(C)]
pub struct Context {
    id: usize,
    state: ContextState,
    block_mutex: Option<Mutex<()>>,
    lock: RwLock<()>,
    cpu: usize
}

impl Context {
    pub fn new(id: usize) -> Context {
        Context {
            id: id,
            state: ContextState::Initializing,
            block_mutex: None,
            lock: RwLock::new(()),
            cpu: usize::MAX
        }
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn is_blocked(&self) -> bool {
        if self.state != ContextState::Blocked {
            return false;
        }
        match &self.block_mutex {
            Some(m) => m.is_locked(),
            _ => false,
        }
    }

    pub fn block(&mut self, mutex: Mutex<()>) {
        let _lock_guard = self.lock.write();
        match &self.block_mutex {
            Some(m) if m.is_locked() => panic!("Attempted to block an already blocked thread!!"),
            _ => self.block_mutex = Some(mutex),
        }
    }

    pub fn activate(&mut self, cpu: usize) {
        self.cpu = cpu;
        self.state = ContextState::Running;
    }

    pub fn save(&mut self) {
        todo!()
    }
}

pub struct Contexts {
    contexts: Vec<Box<Context>>,
    last_id: usize,
}

impl Contexts {
    pub fn select(&mut self, cpu: usize) -> Option<*mut Context> {
        self.suspend(cpu);
        let len = self.contexts.len();
        for i in 0..len {
            let current = self.contexts.get_mut(i).unwrap();
            let ptr = NonNull::from(current.as_ref()).as_ptr();
            unsafe {
                if (*ptr).state == ContextState::Blocked && !(*ptr).is_blocked() {
                    (*ptr).block_mutex = None;
                    (*ptr).state = ContextState::Ready;
                    continue;
                }
                if (*ptr).state == ContextState::Ready {
                    self.contexts.swap(i, len - 1);

                    (*ptr).activate(cpu);

                    return Some(ptr);
                }
            }
        }

        None
    }

    pub fn suspend(&mut self, cpu: usize) {
        for item in self.contexts.iter_mut() {
            let ptr = NonNull::from(item.as_ref()).as_ptr();
            unsafe {
                let lock_handle = (*ptr).lock.write();
                if (*ptr).cpu == cpu {
                    if (*ptr).state == ContextState::Running {
                        (*ptr).state = ContextState::Ready;
                    } else if (*ptr).state == ContextState::Blocked {
                        if !(*ptr).is_blocked() {
                            (*ptr).state = ContextState::Ready;
                            (*ptr).block_mutex = None;
                        }
                    }
                    (*ptr).cpu = usize::MAX;
                }
                drop(lock_handle);
            }
        }
    }

    pub fn create_context(&mut self) -> *mut Context {
        let next = max(
            self.last_id + 1,
            self.contexts
                .iter()
                .map(|context| context.id)
                .max()
                .unwrap_or(self.last_id)
                + 1,
        );
        let mut context = box Context::new(next);
        let ptr = NonNull::<Context>::from(context.as_mut());
        self.last_id = next;
        self.contexts.push(context);
        return ptr.as_ptr();
    }

    pub fn find(&self, id: usize) -> Option<*mut Context> {
        for item in self.contexts.iter() {
            if item.id == id {
                let ptr = NonNull::from(item.as_ref());
                return Some(ptr.as_ptr());
            }
        }
        None
    }
}

lazy_static! {
    pub(crate) static ref CONTEXTS: RwLock<Contexts> = RwLock::new(Contexts {
        contexts: Vec::new(),
        last_id: 0
    });
}
