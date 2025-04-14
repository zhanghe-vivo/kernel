use crate::println;
use bluekernel_test_macro::test;
use semihosting::{c, fs, fs::File, io::Read};

#[test_case]
fn test_semihosting_read_and_write() {
    // FIXME: Do not hard code this path.
    fs::write(c!("/tmp/semihosting.txt"), "What do you want?").unwrap();
    let mut f = File::open(c!("/tmp/semihosting.txt")).unwrap();
    let mut buf = [0u8; 32];
    let len = f.read(&mut buf).unwrap();
    let s = core::str::from_utf8(&buf[..len]).unwrap();
    assert_eq!(s, "What do you want?");
    fs::remove_file(c!("/tmp/semihosting.txt"));
}
