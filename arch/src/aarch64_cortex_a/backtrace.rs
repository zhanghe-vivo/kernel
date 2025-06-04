use crate::{aarch64_cortex_a::Arch, MAX_BACKTRACE_ADDRESSES};
use core::{arch::asm, fmt};
pub struct BacktraceResult {
    addresses: [Option<usize>; MAX_BACKTRACE_ADDRESSES],
}

impl fmt::Display for BacktraceResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\n========== Backtrace: ================\n")?;
        // TODO: Dynamic Identification binary name
        write!(f, "aarch64-none-elf-addr2line -e bluekernel -a ")?;
        for (i, address) in self.addresses.iter().enumerate() {
            if let Some(addr) = address {
                write!(f, " 0x{:08x} ", addr)?;
            }
        }
        write!(f, "\n")?;
        Ok(())
    }
}

impl Arch {
    /// Get an array of backtrace addresses.
    ///
    /// This needs `force-frame-pointers` enabled.
    pub fn backtrace() -> BacktraceResult {
        let fp = unsafe {
            let mut _tmp: u64;
            asm!("mov {0}, x29", out(reg) _tmp);
            _tmp
        };

        backtrace_internal(fp, 2)
    }
}

pub(crate) fn backtrace_internal(fp: u64, suppress: i32) -> BacktraceResult {
    let mut result = BacktraceResult {
        addresses: [None; MAX_BACKTRACE_ADDRESSES],
    };
    let mut index = 0;

    let mut fp = fp;
    let mut current_suppress = suppress;
    let mut old_address = 0;
    loop {
        unsafe {
            let address = (fp as *const u64).offset(1).read_volatile(); // LR/PC

            if address == 0 {
                break;
            }

            fp = (fp as *const u64).read_volatile(); // next FP

            if old_address == address {
                break;
            }

            old_address = address;

            extern "C" {
                static __sys_stack_start: u64;
                static __sys_stack_end: u64;
            }

            let stack_top = &__sys_stack_end as *const u64 as u64;
            let stack_limit =
                (&__sys_stack_end as *const u64 as u64) - (&__sys_stack_start as *const u64 as u64);

            if fp > stack_top || fp < stack_limit {
                break;
            }

            if current_suppress == 0 {
                result.addresses[index] = Some(address as usize);
                index += 1;

                if index >= MAX_BACKTRACE_ADDRESSES {
                    break;
                }
            } else {
                current_suppress -= 1;
            }
        }
    }

    result
}
