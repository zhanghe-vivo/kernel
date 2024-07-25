use crate::{
    idle::IdleThread,
    rt_bindings,
    scheduler::{PriorityTableManager, Scheduler},
    static_init::UnsafeStaticInit,
    sync::RawSpin,
    thread::RtThread,
};
use core::{
    cell::{RefCell, UnsafeCell},
    ptr::NonNull,
    sync::atomic::{AtomicU32, Ordering},
};
use pinned_init::*;

pub const CPUS_NUMBER: usize = rt_bindings::RT_CPUS_NR as usize;
static mut CPUS: UnsafeStaticInit<Cpus, CpusInit> = UnsafeStaticInit::new(CpusInit);

struct CpusInit;
unsafe impl PinInit<Cpus> for CpusInit {
    unsafe fn __pinned_init(self, slot: *mut Cpus) -> Result<(), core::convert::Infallible> {
        let init = Cpus::new();
        unsafe { init.__pinned_init(slot) }
    }
}

// fix error[E0747]: unresolved item provided when a constant was expected
const IDLE_STACK_SIZE: usize = rt_bindings::IDLE_THREAD_STACK_SIZE as usize;

#[pin_data]
pub struct Cpus {
    cpu_lock: RawSpin, // will use as _cpus_lock, cant use SpinLock<T>
    #[pin]
    inner: [Cpu; CPUS_NUMBER],

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

    interrupt_nest: AtomicU32,
    tick: AtomicU32,
    #[pin]
    idle_thread: IdleThread<IDLE_STACK_SIZE>,
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
            global_priority_manager <- PriorityTableManager::new(),
        })
    }

    #[inline]
    pub(crate) fn is_inited() -> bool {
        unsafe { CPUS.is_inited() }
    }

    #[inline]
    pub(crate) fn start_idle_threads() {
        for i in 0..CPUS_NUMBER {
            unsafe { CPUS.inner[i].idle_thread.start() };
        }
    }

    #[cfg(feature = "RT_USING_SMP")]
    #[inline]
    pub(crate) fn get_priority_group_from_global() -> u32 {
        unsafe { CPUS.global_priority_manager.get_priority_group() }
    }

    #[cfg(feature = "RT_USING_SMP")]
    #[inline]
    pub(crate) fn get_highest_priority_from_global() -> u32 {
        debug_assert!(Cpu::get_current_scheduler().is_sched_locked());
        unsafe { CPUS.global_priority_manager.get_highest_ready_prio() }
    }

    #[cfg(feature = "RT_USING_SMP")]
    #[inline]
    pub(crate) fn get_thread_from_global(prio: u32) -> Option<NonNull<RtThread>> {
        debug_assert!(Cpu::get_current_scheduler().is_sched_locked());
        unsafe { CPUS.global_priority_manager.get_thread_by_prio(prio) }
    }

    #[cfg(feature = "RT_USING_SMP")]
    #[inline]
    pub(crate) fn insert_thread_to_global(thread: &mut RtThread) {
        debug_assert!(Cpu::get_current_scheduler().is_sched_locked());
        unsafe { CPUS.global_priority_manager.insert_thread(thread) };
    }

    #[cfg(feature = "RT_USING_SMP")]
    #[inline]
    pub(crate) fn remove_thread_from_global(thread: &mut RtThread) {
        debug_assert!(Cpu::get_current_scheduler().is_sched_locked());
        unsafe { CPUS.global_priority_manager.remove_thread(thread) };
    }

    #[inline]
    pub(crate) fn lock_cpus() {
        unsafe { CPUS.cpu_lock.lock_fast() };
    }

    #[inline]
    pub(crate) fn unlock_cpus() {
        unsafe { CPUS.cpu_lock.unlock_fast() };
    }
}

impl Cpu {
    #[inline]
    pub(crate) fn new(cpu: u8) -> impl PinInit<Self> {
        pin_init!(Self {
            scheduler <- Scheduler::new(cpu),

            interrupt_nest: AtomicU32::new(0),
            tick: AtomicU32::new(0),
            idle_thread <- IdleThread::new(cpu),
        })
    }

    #[inline]
    pub fn get_current() -> &'static Cpu {
        unsafe { &CPUS.inner[rt_bindings::rt_hw_cpu_id() as usize] }
    }

    #[inline]
    pub fn get_by_id(cpu_id: u8) -> &'static Cpu {
        unsafe { &CPUS.inner[cpu_id as usize] }
    }

    #[inline]
    pub fn get_current_mut() -> &'static mut Cpu {
        unsafe { &mut CPUS.inner[rt_bindings::rt_hw_cpu_id() as usize] }
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
    pub fn tick_store(tick: u32) {
        Self::get_current().tick.store(tick, Ordering::Release)
    }

    #[inline]
    pub fn tick_load() -> u32 {
        Self::get_current().tick.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn tick_inc() -> u32 {
        Self::get_current().tick.fetch_add(1, Ordering::Release)
    }

    #[inline]
    pub fn tick_dec() -> u32 {
        Self::get_current().tick.fetch_sub(1, Ordering::Release)
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

/// This function will return the CPU object corresponding to the index.
///
/// # Arguments
///
/// * `index` - the index of the target CPU object.
///
/// # Returns
///
/// Returns a pointer to the CPU object corresponding to the index.
///
// #[no_mangle]
// pub unsafe extern "C" fn rt_cpu_index(index: core::ffi::c_int) -> *mut rt_cpu {
//     #[cfg(feature = "RT_USING_SMP")]
//     return ptr::addr_of_mut!(CPUS[index as usize]);

//     #[cfg(not(feature = "RT_USING_SMP"))]
//     return ptr::addr_of_mut!(CPUS[0]);
// }

/// This function will lock all cpus's scheduler and disable local irq.
/// Return current cpu interrupt status.
#[no_mangle]
pub unsafe extern "C" fn rt_cpus_lock() {
    CPUS.cpu_lock.lock();
}

/// This function will restore all cpus's scheduler and restore local irq.
/// level is interrupt status returned by rt_cpus_lock().
#[no_mangle]
pub unsafe extern "C" fn rt_cpus_unlock(level: rt_bindings::rt_base_t) {
    CPUS.cpu_lock.unlock();
}

/// This function is invoked by scheduler.
/// It will restore the lock state to whatever the thread's counter expects.
/// If target thread not locked the cpus then unlock the cpus lock.
///
/// thread is a pointer to the target thread.
#[no_mangle]
pub extern "C" fn rt_cpus_lock_status_restore(thread: *mut RtThread) {
    assert!(!thread.is_null());

    #[cfg(all(feature = "ARCH_MM_MMU", feature = "RT_USING_SMART"))]
    rt_bindings::lwp_aspace_switch(thread);

    unsafe {
        Cpu::set_current_thread(NonNull::new_unchecked(thread));
    }
    Cpu::get_current_scheduler().ctx_switch_unlock();
}

// need to call before rt_enter_critical/ cpus_lock called
#[no_mangle]
pub unsafe extern "C" fn init_cpus() {
    CPUS.init_once();
}
