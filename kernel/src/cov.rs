// NEWLINE-TIMEOUT: 15
// ASSERT-SUCC: coverage test end.
// ASSERT-FAIL: Backtrace in Panic.*
use minicov::capture_coverage;

pub fn write_coverage_data() {
    let mut cov_data = alloc::vec![];
    unsafe {
        // Note that this function is not thread-safe! Use a lock if needed.
        capture_coverage(&mut cov_data).unwrap();
    }
    use semihosting::{c, fs, io};
    fs::write(c!("output.profraw"), cov_data).unwrap();
    crate::println!("coverage test end.");
}
