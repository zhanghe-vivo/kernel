// TOTAL-TIMEOUT: 8
// ASSERT-SUCC: coverage test end.
// ASSERT-FAIL: Backtrace in Panic.*
use minicov::{CoverageWriteError, CoverageWriter};
use semihosting::{c, fs, io, io::Write};

struct SemihostingCoverageWriter<'a> {
    f: &'a mut fs::File,
}

impl<'a> SemihostingCoverageWriter<'a> {
    pub fn new(f: &'a mut fs::File) -> Self {
        Self { f }
    }
}

impl CoverageWriter for SemihostingCoverageWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> Result<(), CoverageWriteError> {
        let Ok(_) = self.f.write(buf) else {
            return Err(CoverageWriteError);
        };
        Ok(())
    }
}

pub fn write_coverage_data() {
    let mut f = fs::File::create(c"output.profraw").unwrap();
    let mut w = SemihostingCoverageWriter::new(&mut f);
    unsafe {
        // Note that this function is not thread-safe! Use a lock if needed.
        minicov::capture_coverage(&mut w).unwrap();
    }
    f.flush();
    semihosting::println!("coverage test end.");
}
