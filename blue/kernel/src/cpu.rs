#![allow(dead_code)]
use crate::{
    object,
    scheduler::{PriorityTableManager, Scheduler},
    static_init::UnsafeStaticInit,
    sync::RawSpin,
    thread::*,
};
use blue_arch::{arch::Arch, smp, IInterrupt};
use core::{
    ptr::NonNull,
    sync::atomic::{AtomicU32, Ordering},
};
use pinned_init::*;
use rt_bindings;

pub const CPUS_NUMBER: usize = rt_bindings::RT_CPUS_NR as usize;
pub(crate) static mut CPUS: UnsafeStaticInit<Cpus, CpusInit> = UnsafeStaticInit::new(CpusInit);

pub(crate) struct CpusInit;
unsafe impl PinInit<Cpus> for CpusInit {
    unsafe fn __pinned_init(self, slot: *mut Cpus) -> Result<(), core::convert::Infallible> {
        let init = Cpus::new();
        unsafe { init.__pinned_init(slot) }
    }
}

#[pin_data]
pub struct Cpus {
    cpu_lock: RawSpin,
    #[pin]
    inner: [Cpu; CPUS_NUMBER],

    #[cfg(feature = "RT_USING_SMP")]
    sched_lock: RawSpin,
    #[cfg(feature = "RT_USING_SMP")]
    #[pin]
    global_priority_manager: PriorityTableManager,
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
    #[cfg(feature = "RT_USING_SMP")]
    cpu_lock_nest: AtomicU32,
}

impl Cpus {
    #[cfg(not(feature = "RT_USING_SMP"))]
    #[inline]
    pub(crate) fn new() -> impl PinInit<Self> {
        pin_init!(Self {
            cpu_lock: RawSpin::new(),
            inner <- pin_init_array_from_fn(|i| Cpu::new(i as u8)),
        })
    }

    #[cfg(feature = "RT_USING_SMP")]
    #[inline]
    pub(crate) fn new() -> impl PinInit<Self> {
        pin_init!(Self {
            cpu_lock: RawSpin::new(),
            inner <- pin_init_array_from_fn(|i| Cpu::new(i as u8)),
            sched_lock: RawSpin::new(),
            global_priority_manager <- PriorityTableManager::new(),
        })
    }

    #[inline]
    pub(crate) fn is_inited() -> bool {
        unsafe { CPUS.is_inited() }
    }

    #[cfg(feature = "RT_USING_SMP")]
    #[inline]
    pub(crate) fn get_priority_group_from_global() -> u32 {
        unsafe { CPUS.global_priority_manager.get_priority_group() }
    }

    #[cfg(feature = "RT_USING_SMP")]
    #[inline]
    pub(crate) fn get_highest_priority_from_global() -> u32 {
        unsafe { CPUS.global_priority_manager.get_highest_ready_prio() }
    }

    #[cfg(feature = "RT_USING_SMP")]
    #[inline]
    pub(crate) fn get_thread_from_global(prio: u32) -> Option<NonNull<RtThread>> {
        unsafe { CPUS.global_priority_manager.get_thread_by_prio(prio) }
    }

    #[cfg(feature = "RT_USING_SMP")]
    #[inline]
    pub(crate) fn insert_thread_to_global(thread: &mut RtThread) {
        unsafe { CPUS.global_priority_manager.insert_thread(thread) };
    }

    #[cfg(feature = "RT_USING_SMP")]
    #[inline]
    pub(crate) fn remove_thread_from_global(thread: &mut RtThread) {
        unsafe { CPUS.global_priority_manager.remove_thread(thread) };
    }

    #[cfg(feature = "RT_USING_SMP")]
    #[inline]
    pub(crate) fn lock_sched_fast() {
        unsafe { CPUS.sched_lock.lock_fast() };
    }

    #[cfg(feature = "RT_USING_SMP")]
    #[inline]
    pub(crate) fn unlock_sched_fast() {
        unsafe { CPUS.sched_lock.unlock_fast() };
    }

    #[inline]
    pub(crate) fn lock_cpus() {
        #[cfg(feature = "RT_USING_SMP")]
        if Cpu::cpu_lock_nest_inc() == 0 {
            unsafe { CPUS.cpu_lock.lock() };
        }

        #[cfg(not(feature = "RT_USING_SMP"))]
        unsafe {
            CPUS.cpu_lock.lock()
        };
    }

    #[inline]
    pub(crate) fn unlock_cpus() {
        #[cfg(feature = "RT_USING_SMP")]
        if Cpu::cpu_lock_nest_dec() == 1 {
            unsafe { CPUS.cpu_lock.unlock() };
        }

        #[cfg(not(feature = "RT_USING_SMP"))]
        unsafe {
            CPUS.cpu_lock.unlock()
        };
    }
}

impl Cpu {
    #[cfg(not(feature = "RT_USING_SMP"))]
    #[inline]
    pub(crate) fn new(cpu: u8) -> impl PinInit<Self> {
        pin_init!(Self {
            scheduler <- Scheduler::new(cpu),

            interrupt_nest: AtomicU32::new(0),
            tick: AtomicU32::new(0),
        })
    }

    #[cfg(feature = "RT_USING_SMP")]
    #[inline]
    pub(crate) fn new(cpu: u8) -> impl PinInit<Self> {
        pin_init!(Self {
            scheduler <- Scheduler::new(cpu),

            interrupt_nest: AtomicU32::new(0),
            tick: AtomicU32::new(0),
            cpu_lock_nest: AtomicU32::new(0),
        })
    }

    #[inline]
    pub fn get_current() -> &'static Cpu {
        unsafe { &CPUS.inner[smp::core_id::<usize>()] }
    }

    #[inline]
    pub fn get_by_id(cpu_id: u8) -> &'static Cpu {
        unsafe { &CPUS.inner[cpu_id as usize] }
    }

    #[inline]
    pub fn get_current_mut() -> &'static mut Cpu {
        unsafe { &mut CPUS.inner[smp::core_id::<usize>()] }
    }

    #[inline]
    pub fn get_by_id_mut(cpu_id: u8) -> &'static mut Cpu {
        unsafe { &mut CPUS.inner[cpu_id as usize] }
    }

    #[inline]
    pub fn get_current_scheduler() -> &'static mut Scheduler {
        &mut Self::get_current_mut().scheduler
    }

    #[inline]
    pub fn get_scheduler_by_id(cpu_id: u8) -> &'static mut Scheduler {
        debug_assert!(cpu_id < CPUS_NUMBER as u8);
        &mut Self::get_by_id_mut(cpu_id).scheduler
    }

    #[inline]
    pub fn get_current_thread() -> Option<NonNull<RtThread>> {
        Self::get_current_scheduler().get_current_thread()
    }

    #[inline]
    pub fn set_current_thread(th: NonNull<RtThread>) {
        Self::get_current_scheduler().set_current_thread(th);
    }

    #[inline]
    pub fn is_scheduled() -> bool {
        Self::get_current_scheduler().is_scheduled()
    }

    #[inline]
    pub fn tick_store(&self, tick: u32) {
        self.tick.store(tick, Ordering::Release)
    }

    #[inline]
    pub fn tick_load(&self) -> u32 {
        // read tick on cpu 0 only.
        self.tick.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn tick_inc(&self) -> u32 {
        self.tick.fetch_add(1, Ordering::Release)
    }

    #[inline]
    pub fn is_in_interrupt() -> bool {
        Self::get_current().interrupt_nest.load(Ordering::Relaxed) > 0
    }

    #[inline]
    pub fn interrupt_nest_inc() -> u32 {
        Self::get_current()
            .interrupt_nest
            .fetch_add(1, Ordering::Release)
    }

    #[inline]
    pub fn interrupt_nest_dec() -> u32 {
        Self::get_current()
            .interrupt_nest
            .fetch_sub(1, Ordering::Release)
    }

    #[inline]
    pub fn interrupt_nest_load() -> u32 {
        Self::get_current().interrupt_nest.load(Ordering::Relaxed)
    }

    #[cfg(feature = "RT_USING_SMP")]
    #[inline]
    pub fn cpu_lock_nest_inc() -> u32 {
        Self::get_current()
            .cpu_lock_nest
            .fetch_add(1, Ordering::Release)
    }

    #[cfg(feature = "RT_USING_SMP")]
    #[inline]
    pub fn cpu_lock_nest_dec() -> u32 {
        Self::get_current()
            .cpu_lock_nest
            .fetch_sub(1, Ordering::Release)
    }

    #[cfg(feature = "RT_USING_SMP")]
    #[inline]
    pub fn cpu_lock_nest_load() -> u32 {
        Self::get_current().cpu_lock_nest.load(Ordering::Relaxed)
    }
}

// TODO inline to "C"
#[no_mangle]
pub unsafe extern "C" fn rt_interrupt_nest_load() -> u32 {
    Cpu::interrupt_nest_load()
}

#[no_mangle]
pub unsafe extern "C" fn rt_interrupt_nest_inc() -> u32 {
    Cpu::interrupt_nest_inc()
}

#[no_mangle]
pub unsafe extern "C" fn rt_interrupt_nest_dec() -> u32 {
    Cpu::interrupt_nest_dec()
}

/// This function will lock all cpus's scheduler and disable local irq.
/// Return current cpu interrupt status.
#[cfg(feature = "RT_USING_SMP")]
#[no_mangle]
pub unsafe extern "C" fn rt_cpus_lock() -> rt_bindings::rt_base_t {
    let level = Arch::disable_interrupts();
    Cpus::lock_cpus();
    level
}

/// This function will restore all cpus's scheduler and restore local irq.
/// level is interrupt status returned by rt_cpus_lock().
#[cfg(feature = "RT_USING_SMP")]
#[no_mangle]
pub unsafe extern "C" fn rt_cpus_unlock(level: rt_bindings::rt_base_t) {
    Cpus::unlock_cpus();
    Arch::enable_interrupts(level);
}

// need to call before rt_enter_critical/ cpus_lock called
#[no_mangle]
pub extern "C" fn init_cpus() {
    unsafe {
        crate::process::KPROCESS.init_once();
        CPUS.init_once();
        crate::thread::TIDS.init_once();
    }
}

// #[no_mangle]
// pub extern "C" fn rt_cpus_lock_status_restore(thread: *mut RtThread) {
//     assert!(!thread.is_null());
//     let scheduler = Cpu::get_current_scheduler();
//     unsafe {
//         scheduler.set_current_thread(NonNull::new_unchecked(thread));
//     }
//     scheduler.ctx_switch_unlock();
// }
