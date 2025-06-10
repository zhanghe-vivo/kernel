use crate::{
    clock,
    cpu::{Cpu, CPUS_NUMBER},
    error::Error,
    irq::irq_trace::{IrqTraceInfo, IRQ_COUNTS, IRQ_TRACE_INFOS},
    vfs::procfs::*,
};
use alloc::{format, string::String, vec::Vec};
use core::{fmt::Write, sync::atomic::Ordering::Relaxed};
pub struct ProcStatFileOp {
    data: Vec<u8>,
}

impl ProcStatFileOp {
    pub fn new() -> Self {
        ProcStatFileOp { data: Vec::new() }
    }
}

impl ProcNodeOperationTrait for ProcStatFileOp {
    fn get_content(&self) -> Result<Vec<u8>, Error> {
        let cpu_time_str = format_cpu_time();
        let irq_counts_str = format_irq_counts();
        let mut result = String::with_capacity(cpu_time_str.len() + irq_counts_str.len() + 1);
        write!(result, "{}\n{}", cpu_time_str, irq_counts_str).unwrap();
        Ok(result.as_bytes().to_vec())
    }

    fn set_content(&self, content: Vec<u8>) -> Result<(usize), Error> {
        Ok(0)
    }
}

fn format_cpu_time() -> String {
    let mut result = String::with_capacity(100 * (CPUS_NUMBER + 1)); // total + every cpu core
    let mut total_system_time: u64 = 0;
    let mut total_idle_time: u64 = 0;
    let mut total_irq_time: u64 = 0;
    for cpu_id in 0..CPUS_NUMBER {
        let total_cycle: u64 = clock::get_clock_cycle();
        let idle_thread = crate::idle::IdleTheads::get_idle_thread(cpu_id);
        let mut idle_cycle = idle_thread.trace_info.running_cycle;
        if let Some(current_thread) = Cpu::get_scheduler_by_id(cpu_id as u8).get_current_thread() {
            let thread = unsafe { current_thread.as_ref() };
            let idle_last_start_cycle = thread.trace_info.last_running_start_cycle;
            if idle_thread.tid == thread.tid && idle_last_start_cycle > 0 {
                let current_cycle = clock::get_clock_cycle();
                idle_cycle += (current_cycle.saturating_sub(idle_last_start_cycle));
            }
        }
        let system_time = clock::cycle_to_10ms(total_cycle.saturating_sub(idle_cycle));
        let idle_time = clock::cycle_to_10ms(idle_cycle);
        let irq_trace: &IrqTraceInfo = &IRQ_TRACE_INFOS[cpu_id];
        let irq_time = clock::cycle_to_10ms(*(irq_trace.total_irq_process_cycle.read()));
        total_system_time += system_time;
        total_idle_time += idle_time;
        total_irq_time += irq_time;
        write!(
            result,
            "\ncpu{}  {} {} {} {} {} {} {} {} {} {}",
            cpu_id,
            0,           // user
            0,           // nice
            system_time, // system
            idle_time,   // idle
            0,           // iowait
            irq_time,
            0, // softirq
            0, // steal
            0, // guest
            0, // guest_nice
        )
        .unwrap();
    }
    let mut cpu_total_str = String::with_capacity(100);
    write!(
        cpu_total_str,
        "cpu {} {} {} {} {} {} {} {} {} {}",
        0, // user
        0, // nice
        total_system_time,
        total_idle_time,
        0, // iowait
        total_irq_time,
        0, // softirq
        0, // steal
        0, // guest
        0, // guest_nice
    )
    .unwrap();
    result.insert_str(0, &cpu_total_str);
    result
}

fn format_irq_counts() -> String {
    let mut total_count: u64 = 0;
    let mut non_zero_count: usize = 0;
    for atomic in &IRQ_COUNTS {
        let count = atomic.load(Relaxed) as u64;
        total_count = total_count.saturating_add(count);
        if count > 0 {
            non_zero_count += 1;
        }
    }
    const PREFIX: &str = "intr ";
    const U32_MAX_DIGITS: usize = 10;
    const LEN_PER_NON_ZERO_ELEMENT: usize = 1 + U32_MAX_DIGITS; // space + non-zero number
    const LEN_PER_ZERO_ELEMENT: usize = 2; // space + number 0
    let capacity = PREFIX.len()
        + LEN_PER_NON_ZERO_ELEMENT * non_zero_count
        + LEN_PER_ZERO_ELEMENT * (IRQ_COUNTS.len() - non_zero_count);
    let mut result = String::with_capacity(capacity);
    write!(result, "{} {}", PREFIX, total_count).unwrap();
    for element in &IRQ_COUNTS {
        let count = element.load(Relaxed);
        write!(result, " {}", count).unwrap();
    }
    result
}
