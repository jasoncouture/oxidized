#![no_std]
#![no_main]
#![feature(const_mut_refs)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner::test_runner)]
#![reexport_test_harness_main = "test_main"]

mod test_runner;
mod panic;
mod unit_tests;
use kernel_vga_buffer::println;
#[no_mangle]
pub extern "C" fn _start() {
    kernel_main();
}

fn kernel_main() {
    #[cfg(test)]
    test_main();
    println!("Hello World{}", "!");
    panic!("Attempted to exit kernel_main");
}


