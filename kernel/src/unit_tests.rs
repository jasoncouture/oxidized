#[allow(unused_imports)]
use kernel_vga_buffer::{print, println};

#[allow(dead_code)]
#[test_case]
fn trivial_assertion() {
    print!("trivial assertion... ");
    assert_eq!(1, 1);
    println!("[ok]");
}