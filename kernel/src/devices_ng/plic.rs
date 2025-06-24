// TODO: Use safe_mmio.

use crate::arch::riscv64;

pub(crate) type CallbackFn = extern "C" fn(irqno: usize) -> i32;

pub struct Plic {
    base: *mut u32,
}

impl Plic {
    pub const fn new(base: usize) -> Self {
        Self {
            base: unsafe { core::mem::transmute(base) },
        }
    }

    pub fn init(&self) {}

    pub fn set_priority(&self, irq: u32, prio: u32) {
        assert!(irq > 0);
        unsafe { self.base.offset(irq as isize).write_volatile(prio) };
    }

    pub fn enable(&self, irq: u32) {
        let hart = riscv64::current_cpu_id() as isize;
        unsafe {
            let ptr = self
                .base
                .byte_offset(0x2000)
                .byte_offset(hart * 0x80)
                .offset(irq as isize / 32);
            let old = ptr.read_volatile();
            ptr.write_volatile(old | (1 << (irq % 32)));
        }
    }

    pub fn disable(&self, irq: u32) {
        let hart = riscv64::current_cpu_id() as isize;
        unsafe {
            let ptr = self
                .base
                .byte_offset(0x2000)
                .byte_offset(hart * 0x80)
                .offset(irq as isize / 32);
            let old = ptr.read_volatile();
            ptr.write_volatile(old & !(1 << (irq % 32)));
        }
    }

    pub fn claim(&self) -> u32 {
        let hart = riscv64::current_cpu_id() as isize;
        unsafe {
            self.base
                .byte_offset(0x20_0004)
                .byte_offset(hart * 0x1000)
                .read_volatile()
        }
    }

    pub fn complete(&self, irq: u32) {
        let hart = riscv64::current_cpu_id() as isize;
        unsafe {
            self.base
                .byte_offset(0x20_0004)
                .byte_offset(hart * 0x1000)
                .write_volatile(irq)
        }
    }

    pub fn set_threshold(&self, val: u32) {
        let hart = riscv64::current_cpu_id() as isize;
        unsafe {
            self.base
                .byte_offset(0x20_0000)
                .byte_offset(hart * 0x1000)
                .write_volatile(val);
        }
    }
}

unsafe impl Sync for Plic {}
