use crate::{arch::Arch, MAX_BACKTRACE_ADDRESSES};
use core::{arch::asm, fmt};

pub struct BacktraceResult {
    addresses: [Option<usize>; MAX_BACKTRACE_ADDRESSES],
}

impl fmt::Display for BacktraceResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\n========== Backtrace: ================\n")?;
        write!(f, "arm-none-eabi-addr2line -e blue_kernel -a ")?;
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
            let mut _tmp: u32;
            asm!("mov {0}, r7", out(reg) _tmp);
            _tmp
        };

        backtrace_internal(fp, 2)
    }
}

pub(crate) fn backtrace_internal(fp: u32, suppress: i32) -> BacktraceResult {
    let mut result = BacktraceResult {
        addresses: [None; MAX_BACKTRACE_ADDRESSES],
    };
    let mut index = 0;

    let mut fp = fp;
    let mut suppress = suppress;
    let mut old_address = 0;
    loop {
        unsafe {
            let address = (fp as *const u32).offset(1).read_volatile(); // LR/PC
            fp = (fp as *const u32).read_volatile(); // next FP

            if old_address == address {
                break;
            }

            old_address = address;

            if address == 0 {
                break;
            }

            extern "C" {
                static __StackTop: u32;
                static __StackLimit: u32;
            }

            let stack_top = &__StackTop as *const u32 as u32;
            let stack_limit = &__StackLimit as *const u32 as u32;

            if fp > stack_top || fp < stack_limit {
                break;
            }

            if suppress == 0 {
                result.addresses[index] = Some(address as usize);
                index += 1;

                if index >= MAX_BACKTRACE_ADDRESSES {
                    break;
                }
            } else {
                suppress -= 1;
            }
        }
    }

    result
}
