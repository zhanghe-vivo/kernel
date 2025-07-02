#![allow(dead_code)]
use crate::{arch, time};
use bluekernel_kconfig::NUM_CORES;
use core::sync::atomic::{AtomicU32, Ordering};

// nested irq counter
pub(crate) static IRQ_NEST_COUNT: [AtomicU32; NUM_CORES] = [const { AtomicU32::new(0) }; NUM_CORES];

pub struct IrqTrace {
    irq_number: arch::irq::IrqNumber,
}

impl IrqTrace {
    pub fn new(irq_number: arch::irq::IrqNumber) -> Self {
        let irq_trace = Self { irq_number };
        irq_trace.enter();
        irq_trace
    }

    fn enter(&self) {
        let _ = IRQ_NEST_COUNT[arch::current_cpu_id()].fetch_add(1, Ordering::Relaxed);
        #[cfg(procfs)]
        {
            let trace_info: &irq_trace::IrqTraceInfo =
                &irq_trace::IRQ_TRACE_INFOS[arch::current_cpu_id()];
            *(trace_info.last_irq_enter_cycle.write()) = time::get_sys_cycles();
            irq_trace::IRQ_COUNTS[usize::from(self.irq_number)].fetch_add(1, Ordering::Relaxed);
        }
    }

    fn leave(&self) {
        let _ = IRQ_NEST_COUNT[arch::current_cpu_id()].fetch_sub(1, Ordering::Relaxed);
        #[cfg(procfs)]
        {
            let current_cycle = time::get_sys_cycles();
            let trace_info: &irq_trace::IrqTraceInfo =
                &irq_trace::IRQ_TRACE_INFOS[arch::current_cpu_id()];
            let irq_enter_cycle = *(trace_info.last_irq_enter_cycle.read());
            *trace_info.total_irq_process_cycle.write() +=
                current_cycle.saturating_sub(irq_enter_cycle);
        }
    }
}

impl Drop for IrqTrace {
    fn drop(&mut self) {
        self.leave();
    }
}

pub fn is_in_irq() -> bool {
    IRQ_NEST_COUNT[arch::current_cpu_id()].load(Ordering::Relaxed) > 0
}

#[cfg(procfs)]
pub mod irq_trace {
    use crate::arch::irq::INTERRUPT_TABLE_LEN;
    use bluekernel_kconfig::NUM_CORES;
    use core::sync::atomic::AtomicU32;
    use spin::RwLock as SpinRwLock;

    pub static IRQ_COUNTS: [AtomicU32; INTERRUPT_TABLE_LEN] =
        [const { AtomicU32::new(0) }; INTERRUPT_TABLE_LEN];

    pub static IRQ_TRACE_INFOS: [IrqTraceInfo; NUM_CORES] = {
        [const {
            IrqTraceInfo {
                last_irq_enter_cycle: SpinRwLock::new(0),
                total_irq_process_cycle: SpinRwLock::new(0),
            }
        }; NUM_CORES]
    };

    pub struct IrqTraceInfo {
        pub last_irq_enter_cycle: SpinRwLock<u64>,
        pub total_irq_process_cycle: SpinRwLock<u64>,
    }
}
