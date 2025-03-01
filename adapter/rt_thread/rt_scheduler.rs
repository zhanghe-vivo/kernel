use crate::bluekernel::{cpu::Cpu, error::code};

#[cfg(feature = "smp")]
#[no_mangle]
pub extern "C" fn rt_scheduler_ipi_handler() {
    Cpu::get_current_scheduler().do_task_schedule();
}

#[no_mangle]
pub extern "C" fn rt_sched_lock() -> usize {
    Cpu::get_current_scheduler().sched_lock()
}

#[no_mangle]
pub extern "C" fn rt_sched_unlock(level: usize) {
    Cpu::get_current_scheduler().sched_unlock(level);
}

#[no_mangle]
pub extern "C" fn rt_sched_unlock_n_resched(level: usize) -> i32 {
    Cpu::get_current_scheduler().sched_unlock_with_sched(level);
    code::EOK.to_errno()
}

#[no_mangle]
pub extern "C" fn rt_sched_is_locked() -> bool {
    Cpu::get_current_scheduler().is_sched_locked()
}
#[no_mangle]
pub extern "C" fn rt_system_scheduler_init() {}

#[no_mangle]
pub extern "C" fn rt_system_scheduler_start() {
    Cpu::get_current_scheduler().start();
}

#[no_mangle]
pub extern "C" fn rt_schedule() {
    Cpu::get_current_scheduler().do_task_schedule();
}

// Disables preemption for the CPU.
#[no_mangle]
pub extern "C" fn rt_enter_critical() {
    Cpu::get_current_scheduler().preempt_disable();
}

/// Enables scheduler for the CPU.
#[no_mangle]
pub extern "C" fn rt_exit_critical() {
    Cpu::get_current_scheduler().preempt_enable();
}

#[no_mangle]
pub extern "C" fn rt_critical_level() -> u32 {
    Cpu::get_current_scheduler().get_sched_lock_level()
}
