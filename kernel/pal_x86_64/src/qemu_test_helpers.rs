pub fn exit_qemu(exit_code: u32) {
    use x86_64::instructions::port::Port;

    unsafe {
        // Special QEMU port for communicating exit codes.
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}