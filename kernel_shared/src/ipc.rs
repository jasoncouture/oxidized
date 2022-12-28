#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct Server(u128);
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct InterProcessData(usize);

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct InterProcessCallMetadata {
    server: Server,
    function: u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct InterProcessCall {
    metadata: InterProcessCallMetadata,
    parameters_address: InterProcessData,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct InterProcessResult {
    metadata: InterProcessCallMetadata,
    result_address: InterProcessData,
}

pub trait Pointer {
    fn as_ptr(&self) -> *const u8 {
        self.as_mut_ptr() as *const u8
    }

    fn as_mut_ptr(&self) -> *mut u8;
}

impl Pointer for InterProcessData {
    fn as_mut_ptr(&self) -> *mut u8 {
        self.0 as *mut u8
    }
}

impl Pointer for InterProcessCall {
    fn as_mut_ptr(&self) -> *mut u8 {
        self.parameters_address.as_mut_ptr()
    }
}

impl Pointer for InterProcessResult {
    fn as_mut_ptr(&self) -> *mut u8 {
        self.result_address.as_mut_ptr()
    }
}
