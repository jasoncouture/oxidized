use crate::constants::*;

#[derive(Debug, Clone, Copy)]
pub struct Handle {
    identifier: usize,
    server_process: usize,
    process: usize,
}


