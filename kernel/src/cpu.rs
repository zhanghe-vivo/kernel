#![allow(dead_code)]
#[cfg(feature = "smp")]
use crate::scheduler::PriorityTableManager;
use crate::{
    arch::Arch, bluekernel_kconfig::CPUS_NR, process, scheduler::Scheduler,
    static_init::UnsafeStaticInit, sync::RawSpin, thread,
};

use core::{
    ptr::NonNull,
    sync::atomic::{AtomicU32, Ordering},
};
use pinned_init::{pin_data, pin_init, pin_init_array_from_fn, PinInit};

pub const CPUS_NUMBER: usize = CPUS_NR as usize;
pub const CPU_DETACHED: u8 = CPUS_NUMBER as u8;
pub(crate) static mut CPUS: UnsafeStaticInit<Cpus, CpusInit> = UnsafeStaticInit::new(CpusInit);

pub(crate) struct CpusInit;
unsafe impl PinInit<Cpus> for CpusInit {
    unsafe fn __pinned_init(self, slot: *mut Cpus) -> Result<(), core::convert::Infallible> {
        let init = Cpus::new();
        unsafe { init.__pinned_init(slot) }
    }
}

#[cfg(feature = "smp")]
#[pin_data]
pub struct Cpus {
    cpu_lock: RawSpin,
    #[pin]
    inner: [Cpu; CPUS_NUMBER],

    sched_lock: RawSpin,
    #[pin]
    global_priority_manager: PriorityTableManager,
}

#[cfg(not(feature = "smp"))]
#[pin_data]
pub struct Cpus {
    cpu_lock: RawSpin,
    #[pin]
    inner: [Cpu; CPUS_NUMBER],
}

// cant Send
unsafe impl Sync for Cpus {}

#[pin_data]
pub struct Cpu {
    /// scheduler for per cpu
    #[pin]
    scheduler: Scheduler,

    tick: AtomicU32,
    interrupt_nest: AtomicU32,
    #[cfg(feature = "smp")]
    cpu_lock_nest: AtomicU32,
}

impl Cpus {
    #[cfg(not(feature = "smp"))]
    #[inline]
    pub(crate) fn new() -> impl PinInit<Self> {
        pin_init!(Self {
            cpu_lock: RawSpin::new(),
            inner <- pin_init_array_from_fn(|i| Cpu::new(i as u8)),
        })
    }

    #[cfg(feature = "smp")]
    #[inline]
    pub(crate) fn new() -> impl PinInit<Self> {
        pin_init!(Self {
            cpu_lock: RawSpin::new(),
            inner <- pin_init_array_from_fn(|i| Cpu::new(i as u8)),
            sched_lock: RawSpin::new(),
            global_priority_manager <- PriorityTableManager::new(),
        })
    }

    #[cfg(feature = "smp")]
    #[inline]
    pub(crate) fn get_priority_group_from_global() -> u32 {
        let cpus = unsafe { &*(&raw const CPUS as *const UnsafeStaticInit<Cpus, CpusInit>) };
        cpus.global_priority_manager.get_priority_group()
    }

    #[cfg(feature = "smp")]
    #[inline]
    pub(crate) fn get_highest_priority_from_global() -> u32 {
        let cpus = unsafe { &*(&raw const CPUS as *const UnsafeStaticInit<Cpus, CpusInit>) };
        cpus.global_priority_manager.get_highest_ready_prio()
    }

    #[cfg(feature = "smp")]
    #[inline]
    pub(crate) fn get_thread_from_global(prio: u32) -> Option<NonNull<thread::Thread>> {
        let cpus = unsafe { &*(&raw const CPUS as *const UnsafeStaticInit<Cpus, CpusInit>) };
        cpus.global_priority_manager.get_thread_by_prio(prio)
    }

    #[cfg(feature = "smp")]
    #[inline]
    pub(crate) fn insert_thread_to_global(thread: &mut thread::Thread) {
        let cpus = unsafe { &*(&raw const CPUS as *const UnsafeStaticInit<Cpus, CpusInit>) };
        cpus.global_priority_manager.insert_thread(thread);
    }

    #[cfg(feature = "smp")]
    #[inline]
    pub(crate) fn remove_thread_from_global(thread: &mut thread::Thread) {
        let cpus = unsafe { &*(&raw const CPUS as *const UnsafeStaticInit<Cpus, CpusInit>) };
        cpus.global_priority_manager.remove_thread(thread);
    }

    #[cfg(feature = "smp")]
    #[inline]
    pub(crate) fn lock_sched_fast() {
        let cpus = unsafe { &*(&raw const CPUS as *const UnsafeStaticInit<Cpus, CpusInit>) };
        cpus.sched_lock.lock_fast();
    }

    #[cfg(feature = "smp")]
    #[inline]
    pub(crate) fn unlock_sched_fast() {
        let cpus = unsafe { &*(&raw const CPUS as *const UnsafeStaticInit<Cpus, CpusInit>) };
        cpus.sched_lock.unlock_fast();
    }

    #[inline]
    pub(crate) fn is_inited() -> bool {
        let cpus = unsafe { &*(&raw const CPUS as *const UnsafeStaticInit<Cpus, CpusInit>) };
        cpus.is_inited()
    }

    #[inline]
    pub(crate) fn lock_cpus() {
        let cpus = unsafe { &*(&raw const CPUS as *const UnsafeStaticInit<Cpus, CpusInit>) };
        #[cfg(feature = "smp")]
        if Cpu::cpu_lock_nest_inc() == 0 {
            cpus.cpu_lock.lock();
        }

        #[cfg(not(feature = "smp"))]
        {
            cpus.cpu_lock.lock();
        }
    }

    #[inline]
    pub(crate) fn unlock_cpus() {
        let cpus = unsafe { &*(&raw const CPUS as *const UnsafeStaticInit<Cpus, CpusInit>) };
        #[cfg(feature = "smp")]
        if Cpu::cpu_lock_nest_dec() == 1 {
            cpus.cpu_lock.unlock();
        }

        #[cfg(not(feature = "smp"))]
        {
            cpus.cpu_lock.unlock();
        }
    }
}

impl Cpu {
    #[cfg(not(feature = "smp"))]
    pub(crate) fn new(cpu: u8) -> impl PinInit<Self> {
        pin_init!(Self {
            scheduler <- Scheduler::new(cpu),

            interrupt_nest: AtomicU32::new(0),
            tick: AtomicU32::new(0),
        })
    }

    #[cfg(feature = "smp")]
    pub(crate) fn new(cpu: u8) -> impl PinInit<Self> {
        pin_init!(Self {
            scheduler <- Scheduler::new(cpu),

            interrupt_nest: AtomicU32::new(0),
            tick: AtomicU32::new(0),
            cpu_lock_nest: AtomicU32::new(0),
        })
    }

    #[inline(always)]
    pub fn get_current() -> &'static Cpu {
        unsafe { &CPUS.inner[Arch::core_id::<usize>()] }
    }

    #[inline(always)]
    pub fn get_by_id(cpu_id: u8) -> &'static Cpu {
        unsafe { &CPUS.inner[cpu_id as usize] }
    }

    #[inline(always)]
    pub fn get_current_mut() -> &'static mut Cpu {
        unsafe { &mut CPUS.inner[Arch::core_id::<usize>()] }
    }

    #[inline(always)]
    pub fn get_by_id_mut(cpu_id: u8) -> &'static mut Cpu {
        unsafe { &mut CPUS.inner[cpu_id as usize] }
    }

    #[inline(always)]
    pub fn get_current_scheduler() -> &'static mut Scheduler {
        &mut Self::get_current_mut().scheduler
    }

    #[inline(always)]
    pub fn get_scheduler_by_id(cpu_id: u8) -> &'static mut Scheduler {
        debug_assert!(cpu_id < CPUS_NUMBER as u8);
        &mut Self::get_by_id_mut(cpu_id).scheduler
    }

    #[inline(always)]
    pub fn get_current_thread() -> Option<NonNull<thread::Thread>> {
        Self::get_current_scheduler().get_current_thread()
    }

    #[inline(always)]
    pub fn set_current_thread(th: NonNull<thread::Thread>) {
        Self::get_current_scheduler().set_current_thread(th);
    }

    #[inline(always)]
    pub fn is_scheduled() -> bool {
        Self::get_current_scheduler().is_scheduled()
    }

    #[inline(always)]
    pub fn tick_store(&self, tick: u32) {
        self.tick.store(tick, Ordering::Relaxed)
    }

    #[inline(always)]
    pub fn tick_load(&self) -> u32 {
        // read tick on cpu 0 only.
        self.tick.load(Ordering::Relaxed)
    }

    #[inline(always)]
    pub fn tick_inc(&self) -> u32 {
        self.tick.fetch_add(1, Ordering::Relaxed)
    }

    #[inline(always)]
    pub fn is_in_interrupt() -> bool {
        Self::get_current().interrupt_nest.load(Ordering::Acquire) > 0
    }

    #[inline(always)]
    pub fn interrupt_nest_inc() -> u32 {
        Self::get_current()
            .interrupt_nest
            .fetch_add(1, Ordering::Release)
    }

    #[inline(always)]
    pub fn interrupt_nest_dec() -> u32 {
        Self::get_current()
            .interrupt_nest
            .fetch_sub(1, Ordering::Release)
    }

    #[inline(always)]
    pub fn interrupt_nest_load() -> u32 {
        Self::get_current().interrupt_nest.load(Ordering::Acquire)
    }

    #[cfg(feature = "smp")]
    #[inline(always)]
    pub fn cpu_lock_nest_inc() -> u32 {
        Self::get_current()
            .cpu_lock_nest
            .fetch_add(1, Ordering::Release)
    }

    #[cfg(feature = "smp")]
    #[inline(always)]
    pub fn cpu_lock_nest_dec() -> u32 {
        Self::get_current()
            .cpu_lock_nest
            .fetch_sub(1, Ordering::Release)
    }

    #[cfg(feature = "smp")]
    #[inline(always)]
    pub fn cpu_lock_nest_load() -> u32 {
        Self::get_current().cpu_lock_nest.load(Ordering::Acquire)
    }
}

// need to call before disables preemption for the CPU/ cpus_lock called
#[no_mangle]
pub extern "C" fn init_cpus() {
    unsafe {
        let process = &*(&raw const process::KPROCESS
            as *const UnsafeStaticInit<process::Kprocess, process::KprocessInit>);
        let cpus = &*(&raw const CPUS as *const UnsafeStaticInit<Cpus, CpusInit>);
        let tid =
            &*(&raw const thread::TIDS as *const UnsafeStaticInit<thread::Tid, thread::TidInit>);
        process.init_once();
        cpus.init_once();
        tid.init_once();
    }
}
