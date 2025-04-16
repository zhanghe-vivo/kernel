#![no_std]

use cargo_kconfig::{kbool, kint, load_kcfg};
use enum_iterator::Sequence;

load_kcfg!(
    kconfig_path = "../../kernel/kconfig/config",
    dotconfig_path = "../../kernel/kconfig/config",
);

pub const ALIGN_SIZE: usize = kint!(ALIGN_SIZE) as usize;

pub const TICK_PER_SECOND: u32 = kint!(TICK_PER_SECOND) as u32;

pub const MAIN_THREAD_PRIORITY: u32 = kint!(MAIN_THREAD_PRIORITY) as u32;

pub const MAIN_THREAD_STACK_SIZE: u32 = kint!(MAIN_THREAD_STACK_SIZE) as u32;

pub const THREAD_PRIORITY_MAX: u32 = kint!(THREAD_PRIORITY_MAX) as u32;

pub const IDLE_THREAD_STACK_SIZE: u32 = kint!(IDLE_THREAD_STACK_SIZE) as u32;

pub const SERIAL_RX_FIFO_SIZE: usize = kint!(SERIAL_RX_FIFO_SIZE) as usize;

pub const SERIAL_TX_FIFO_SIZE: usize = kint!(SERIAL_TX_FIFO_SIZE) as usize;

pub const CPUS_NR: u32 = get_cpus_nr();

const fn get_cpus_nr() -> u32 {
    if kbool!(SMP) {
        kint!(CPUS_NR) as u32
    } else {
        1
    }
}

#[derive(Debug, PartialEq, Sequence)]
pub enum Feature {
    Smp,
    Tlsf,
    OverflowCheck,
    IdleHook,
    Heap,
    HeapIsr,
    DebuggingInit,
    Event,
    MessageQueue,
    Mailbox,
    Mutex,
    Semaphore,
    Rwlock,
    Condvar,
    CompatNewlibc,
    ThreadPriorityMax,
}

impl Feature {
    pub fn is_enabled(&self) -> bool {
        match &self {
            Feature::Smp => kbool!(SMP),
            Feature::Tlsf => kbool!(TLSF),
            Feature::OverflowCheck => kbool!(OVERFLOW_CHECK),
            Feature::IdleHook => kbool!(IDLE_HOOK),
            Feature::Heap => kbool!(HEAP),
            Feature::HeapIsr => kbool!(HEAP_ISR),
            Feature::DebuggingInit => kbool!(DEBUGGING_INIT),
            Feature::Event => kbool!(EVENT),
            Feature::MessageQueue => kbool!(MESSAGEQUEUE),
            Feature::Mailbox => kbool!(MAILBOX),
            Feature::Mutex => kbool!(MUTEX),
            Feature::Semaphore => kbool!(SEMAPHORE),
            Feature::Rwlock => kbool!(RWLOCK),
            Feature::Condvar => kbool!(CONDVAR),
            Feature::CompatNewlibc => kbool!(COMPAT_NEWLIBC),
            Feature::ThreadPriorityMax => THREAD_PRIORITY_MAX >= 32,
        }
    }

    pub fn to_string(&self) -> &str {
        match &self {
            Feature::Smp => "smp",
            Feature::Tlsf => "tlsf",
            Feature::OverflowCheck => "overflow_check",
            Feature::IdleHook => "idle_hook",
            Feature::Heap => "heap",
            Feature::HeapIsr => "heap_isr",
            Feature::DebuggingInit => "debugging_init",
            Feature::Event => "event",
            Feature::MessageQueue => "messagequeue",
            Feature::Mailbox => "mailbox",
            Feature::Mutex => "mutex",
            Feature::Semaphore => "semaphore",
            Feature::Rwlock => "rwlock",
            Feature::Condvar => "condvar",
            Feature::CompatNewlibc => "compat_newlibc",
            Feature::ThreadPriorityMax => "thread_priority_max",
        }
    }
}
