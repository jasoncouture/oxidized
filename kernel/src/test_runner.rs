#[cfg(test)]
pub fn test_runner(tests: &[&dyn Fn()]) {
    use crate::println;

    println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
}