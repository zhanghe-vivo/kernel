#![allow(dead_code)]
#[cfg(procfs)]
use crate::clock;
use crate::{
    arch::{interrupt::IrqNumber, Arch},
    cpu::Cpu,
};
use core::{
    cell::{Cell, UnsafeCell},
    ops::{Deref, DerefMut},
};

#[repr(transparent)]
#[derive(Clone, Debug)]
pub struct IrqLockRaw(Cell<usize>);

impl IrqLockRaw {
    #[inline]
    pub const fn new() -> Self {
        Self(Cell::new(0))
    }

    #[inline]
    pub fn lock(&self) -> IrqLockRawGuard<'_> {
        self.raw_lock();
        IrqLockRawGuard(self)
    }

    #[inline]
    fn raw_lock(&self) {
        self.0.replace(Arch::disable_interrupts());
    }

    #[inline]
    fn raw_unlock(&self) {
        Arch::enable_interrupts(self.0.get());
    }
}

pub struct IrqLockRawGuard<'a>(&'a IrqLockRaw);

impl Drop for IrqLockRawGuard<'_> {
    #[inline]
    fn drop(&mut self) {
        self.0.raw_unlock();
    }
}

pub struct IrqLock<T> {
    lock: IrqLockRaw,
    inner: UnsafeCell<T>,
}

impl<T> IrqLock<T> {
    pub const fn new(element: T) -> Self {
        IrqLock {
            lock: IrqLockRaw::new(),
            inner: UnsafeCell::new(element),
        }
    }

    pub fn lock(&self) -> IrqGuard<'_, T> {
        self.raw_lock();
        IrqGuard::new(&self)
    }

    #[inline(always)]
    fn raw_lock(&self) {
        self.lock.raw_lock();
    }

    #[inline(always)]
    fn raw_unlock(&self) {
        self.lock.raw_unlock();
    }
}

unsafe impl<T> Sync for IrqLock<T> {}

pub struct IrqGuard<'a, T> {
    lock: &'a IrqLock<T>,
}

impl<'a, T> IrqGuard<'a, T> {
    fn new(lock: &'a IrqLock<T>) -> Self {
        IrqGuard { lock }
    }
}

impl<'a, T> Deref for IrqGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.inner.get() }
    }
}

impl<'a, T> DerefMut for IrqGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.inner.get() }
    }
}

impl<'a, T> Drop for IrqGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.raw_unlock();
    }
}

pub struct Irq;

impl Irq {
    #[allow(unused_variables)]
    pub fn enter(irq_number: IrqNumber) {
        let last_nest = Cpu::interrupt_nest_inc();
        #[cfg(procfs)]
        {
            use core::sync::atomic::Ordering;
            if last_nest == 0 {
                let trace_info: &irq_trace::IrqTraceInfo =
                    &irq_trace::IRQ_TRACE_INFOS[Arch::core_id::<usize>()];
                *(trace_info.last_irq_enter_cycle.write()) = clock::get_clock_cycle();
            }
            irq_trace::IRQ_COUNTS[usize::from(irq_number)].fetch_add(1, Ordering::Relaxed);
        }
    }

    #[allow(unused_variables)]
    pub fn leave() {
        let last_nest = Cpu::interrupt_nest_dec();
        #[cfg(procfs)]
        {
            if last_nest == 1 {
                let current_cycle = clock::get_clock_cycle();
                let trace_info: &irq_trace::IrqTraceInfo =
                    &irq_trace::IRQ_TRACE_INFOS[Arch::core_id::<usize>()];
                let irq_enter_cycle = *(trace_info.last_irq_enter_cycle.read());
                *trace_info.total_irq_process_cycle.write() +=
                    current_cycle.saturating_sub(irq_enter_cycle);
            }
        }
    }
}

#[cfg(procfs)]
pub mod irq_trace {
    use crate::{arch::interrupt::INTERRUPT_TABLE_LEN, cpu::CPUS_NUMBER};
    use core::sync::atomic::AtomicU32;
    use spin::RwLock as SpinRwLock;

    pub static IRQ_COUNTS: [AtomicU32; INTERRUPT_TABLE_LEN] =
        { [const { AtomicU32::new(0) }; INTERRUPT_TABLE_LEN] };

    pub static IRQ_TRACE_INFOS: [IrqTraceInfo; CPUS_NUMBER] = {
        [const {
            IrqTraceInfo {
                last_irq_enter_cycle: SpinRwLock::new(0),
                total_irq_process_cycle: SpinRwLock::new(0),
            }
        }; CPUS_NUMBER]
    };

    pub struct IrqTraceInfo {
        pub last_irq_enter_cycle: SpinRwLock<u64>,
        pub total_irq_process_cycle: SpinRwLock<u64>,
    }
}
