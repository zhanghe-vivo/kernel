use crate::cpu::Cpus;
use crate::{cpu::Cpu, thread::RtThread};
use blue_arch::arch::Arch;
use blue_arch::{ICore, IInterrupt, IScheduler};
use blue_infra::list::doubly_linked_list::ListHead;

#[cfg(feature = "RT_USING_SMP")]
use crate::{cpu::CPUS_NUMBER, sync::RawSpin};

#[cfg(feature = "debug_scheduler")]
use crate::{println, str::CStr};

use core::{
    ffi,
    intrinsics::likely,
    pin::Pin,
    ptr::{self, NonNull},
    sync::atomic::{AtomicPtr, AtomicU32, Ordering},
};
use pinned_init::*;
use rt_bindings;

#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
type RtSchedulerHook = extern "C" fn(from: *mut RtThread, to: *mut RtThread);
#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
type RtSchedulerSwitchHook = extern "C" fn(tid: *mut RtThread);
#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
static mut RT_SCHEDULER_HOOK: Option<RtSchedulerHook> = None;
#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
static mut RT_SCHEDULER_SWITCH_HOOK: Option<RtSchedulerSwitchHook> = None;

// #[repr(C)]
#[pin_data]
pub struct PriorityTableManager {
    #[pin]
    priority_table: [ListHead; rt_bindings::RT_THREAD_PRIORITY_MAX as usize],

    priority_group: u32,
    // FIXME #[cfg(RT_THREAD_PRIORITY_MAX > 32)]
    ready_table: [u8; 32],
}

// #[repr(C)]
#[pin_data]
pub struct Scheduler {
    pub(crate) current_thread: AtomicPtr<RtThread>,
    // Scheduler lock as local irq, not need spin_lock
    ///priority list headers
    #[pin]
    priority_manager: PriorityTableManager,

    scheduler_lock_nest: AtomicU32,
    id: u8,
    current_priority: u8,
    irq_switch_flag: u8,
    critical_switch_flag: u8,
    sched_lock_flag: u8,
}

impl PriorityTableManager {
    pub fn new() -> impl PinInit<Self> {
        pin_init!(Self {
            priority_table <- pin_init_array_from_fn(|_| ListHead::new()),
            priority_group: 0,
            ready_table: [0; 32],
        })
    }

    #[inline]
    pub fn get_priority_group(&self) -> u32 {
        self.priority_group
    }

    // FIXME #[cfg(RT_THREAD_PRIORITY_MAX > 32)]
    #[inline]
    pub fn get_highest_ready_prio(&self) -> u32 {
        let num = self.priority_group.trailing_zeros();
        if num != u32::MAX {
            return (num << 3) + self.ready_table[num as usize].trailing_zeros();
        }
        num
    }
    // FIXME #[cfg(not(RT_THREAD_PRIORITY_MAX > 32))]
    // #[inline]
    // pub fn get_highest_ready_prio(&self) -> u32 {
    //     (unsafe { rt_bindings::__rt_ffs(self.priority_group) } - 1) as u32
    // }

    pub fn get_thread_by_prio(&self, prio: u32) -> Option<NonNull<RtThread>> {
        if let Some(node) = self.priority_table[prio as usize].next() {
            unsafe {
                let thread: *mut RtThread = crate::thread_list_node_entry!(node.as_ptr());
                return Some(NonNull::new_unchecked(thread));
            }
        }
        None
    }

    pub fn insert_thread(&mut self, thread: &mut RtThread) {
        debug_assert!(thread.tlist.is_empty());
        // FIXME #[cfg(RT_THREAD_PRIORITY_MAX > 32)]
        self.ready_table[thread.number as usize] |= thread.high_mask;
        self.priority_group |= thread.number_mask;

        /* there is no time slices left(YIELD), inserting thread before ready list*/
        if thread.is_yield() {
            unsafe {
                Pin::new_unchecked(&mut thread.tlist)
                    .insert_prev(&self.priority_table[thread.current_priority as usize])
            };
        } else {
            unsafe {
                Pin::new_unchecked(&mut thread.tlist)
                    .insert_next(&self.priority_table[thread.current_priority as usize])
            };
        }
    }

    pub fn remove_thread(&mut self, thread: &mut RtThread) {
        unsafe { Pin::new_unchecked(&mut thread.tlist).remove() };

        if self.priority_table[thread.current_priority as usize].is_empty() {
            // FIXME #[cfg(RT_THREAD_PRIORITY_MAX > 32)]
            self.ready_table[thread.number as usize] &= !thread.high_mask;
            if self.ready_table[thread.number as usize] == 0 {
                self.priority_group &= !(thread.number_mask);
            }
            // FIXME #[cfg(not(RT_THREAD_PRIORITY_MAX > 32))]
            // self.priority_group &= !(thread.number_mask);
        }
    }
}

impl Scheduler {
    pub fn new(index: u8) -> impl PinInit<Self> {
        pin_init!(Self {
            current_thread: AtomicPtr::new(ptr::null_mut()),
            priority_manager <- PriorityTableManager::new(),
            scheduler_lock_nest: AtomicU32::new(0),
            id: index,
            current_priority: (rt_bindings::RT_THREAD_PRIORITY_MAX - 1) as u8,
            irq_switch_flag: 0,
            critical_switch_flag: 0,
            sched_lock_flag: 0,
        })
    }

    #[inline]
    pub const fn get_current_id(&self) -> u8 {
        self.id
    }

    #[inline]
    pub fn get_current_thread(&self) -> Option<NonNull<RtThread>> {
        NonNull::new(self.current_thread.load(Ordering::Relaxed))
    }

    #[inline]
    pub fn set_current_thread(&self, th: NonNull<RtThread>) {
        self.current_thread.store(th.as_ptr(), Ordering::Release);
    }

    #[inline]
    pub fn is_scheduled(&self) -> bool {
        self.current_thread.load(Ordering::Relaxed) != ptr::null_mut()
    }

    #[inline]
    pub fn preempt_disable(&self) {
        if likely(self.is_scheduled()) {
            self.scheduler_lock_nest.fetch_add(1, Ordering::Release);
        }
    }

    #[inline]
    pub fn preempt_enable(&mut self) {
        if likely(self.is_scheduled()) {
            debug_assert!(self.scheduler_lock_nest.load(Ordering::Relaxed) > 0);
            let level = Arch::disable_interrupts();
            if self.scheduler_lock_nest.fetch_sub(1, Ordering::Release) == 1 {
                if self.critical_switch_flag == 1 {
                    self.do_task_schedule();
                }
            }
            Arch::enable_interrupts(level);
        }
    }

    #[cfg(feature = "RT_USING_SMP")]
    #[inline]
    fn sched_lock_mp(&mut self) {
        debug_assert!(self.sched_lock_flag == 0);
        Cpus::lock_sched_fast();
        self.sched_lock_flag = 1;
    }

    #[cfg(feature = "RT_USING_SMP")]
    #[inline]
    fn sched_unlock_mp(&mut self) {
        debug_assert!(self.sched_lock_flag == 1);
        self.sched_lock_flag = 0;
        Cpus::unlock_sched_fast();
    }

    fn get_highest_priority_thread_locked(&self) -> Option<(NonNull<RtThread>, u32)> {
        debug_assert!(self.is_sched_locked());

        let highest = self.priority_manager.get_highest_ready_prio();

        #[cfg(feature = "RT_USING_SMP")]
        {
            let global_highest = Cpus::get_highest_priority_from_global();
            if global_highest < highest {
                let thread = Cpus::get_thread_from_global(global_highest);
                match thread {
                    Some(th) => return Some((th, global_highest)),
                    None => {
                        return None;
                    }
                }
            }
        }

        if highest != u32::MAX {
            let thread = self.priority_manager.get_thread_by_prio(highest);
            match thread {
                Some(th) => return Some((th, highest)),
                None => {
                    return None;
                }
            }
        }

        None
    }

    pub fn insert_thread_locked(&mut self, thread: &mut RtThread) {
        debug_assert!(self.is_sched_locked());

        if thread.is_ready() {
            return;
        }

        #[cfg(feature = "RT_USING_SMP")]
        if !thread.is_cpu_detached() {
            // only YIELD -> READY, SUSPEND -> READY is allowed by this API. However,
            // this is a RUNNING thread. So here we reset it's status and let it go.
            thread.set_running();
            return;
        }

        // current thread is changed in rt_cpus_lock_status_restore now. cant let it go
        // #[cfg(not(feature = "RT_USING_SMP"))]
        // if thread.is_current_runnung_thread() {
        //     // only YIELD -> READY, SUSPEND -> READY is allowed by this API. However,
        //     // this is a RUNNING thread. So here we reset it's status and let it go.
        //     thread.set_running();
        //     return;
        // }

        thread.set_ready();

        #[cfg(not(feature = "RT_USING_SMP"))]
        self.priority_manager.insert_thread(thread);

        #[cfg(feature = "RT_USING_SMP")]
        {
            let cpu_id = self.get_current_id();
            let bind_cpu = thread.get_bind_cpu();
            if bind_cpu == CPUS_NUMBER as u8 {
                Cpus::insert_thread_to_global(thread);
                let cpu_mask = rt_bindings::RT_CPU_MASK ^ (1 << cpu_id);
                unsafe {
                    rt_bindings::rt_hw_ipi_send(rt_bindings::RT_SCHEDULE_IPI as i32, cpu_mask)
                };
            } else {
                if bind_cpu == cpu_id {
                    self.priority_manager.insert_thread(thread);
                } else {
                    Cpu::get_scheduler_by_id(bind_cpu)
                        .priority_manager
                        .insert_thread(thread);
                    let cpu_mask = rt_bindings::RT_CPU_MASK ^ (1 << cpu_id);
                    unsafe {
                        rt_bindings::rt_hw_ipi_send(rt_bindings::RT_SCHEDULE_IPI as i32, cpu_mask)
                    };
                }
            }
        }
    }

    pub fn remove_thread_locked(&mut self, thread: &mut RtThread) {
        debug_assert!(self.is_sched_locked());

        #[cfg(not(feature = "RT_USING_SMP"))]
        self.priority_manager.remove_thread(thread);

        #[cfg(feature = "RT_USING_SMP")]
        {
            let bind_cpu = thread.get_bind_cpu();
            if bind_cpu == CPUS_NUMBER as u8 {
                Cpus::remove_thread_from_global(thread);
            } else {
                Cpu::get_scheduler_by_id(bind_cpu)
                    .priority_manager
                    .remove_thread(thread);
            }
        }
    }

    pub fn change_priority_locked(&mut self, thread: &mut RtThread, priority: u8) {
        debug_assert!(self.is_scheduled());
        debug_assert!(self.is_sched_locked());
        assert!(priority < rt_bindings::RT_THREAD_PRIORITY_MAX as u8);

        if thread.is_ready() {
            self.remove_thread_locked(thread);
            thread.set_priority(priority);
            thread.set_init_stat();
            self.insert_thread_locked(thread);
        } else {
            thread.set_priority(priority);
        }
    }

    #[inline]
    fn has_ready_thread(&self) -> bool {
        #[cfg(not(feature = "RT_USING_SMP"))]
        return self.priority_manager.priority_group != 0;

        #[cfg(feature = "RT_USING_SMP")]
        return Cpus::get_priority_group_from_global() != 0
            || self.priority_manager.priority_group != 0;
    }

    fn prepare_context_switch_locked(&mut self) -> Option<NonNull<RtThread>> {
        /* quickly check if any other ready threads queuing */
        if self.has_ready_thread() {
            let to_thread = self.get_highest_priority_thread_locked();
            match to_thread {
                Some((mut new_thread, highest_ready_priority)) => {
                    debug_assert!(self.get_current_thread() != None);
                    // cur_th must not be Null here
                    let cur_th = unsafe { self.get_current_thread().unwrap_unchecked().as_mut() };
                    #[cfg(feature = "RT_USING_SMP")]
                    let cpu_id = self.get_current_id();

                    /* check if current thread can be running on current core again */
                    if cur_th.is_running() {
                        let switch_current = cur_th.current_priority < highest_ready_priority as u8
                            || (cur_th.current_priority == highest_ready_priority as u8
                                && (cur_th.stat & rt_bindings::RT_THREAD_STAT_YIELD_MASK as u8)
                                    == 0);
                        #[cfg(feature = "RT_USING_SMP")]
                        let some_cpu = cur_th.get_bind_cpu() == CPUS_NUMBER as u8
                            || cur_th.get_bind_cpu() == cpu_id;
                        #[cfg(feature = "RT_USING_SMP")]
                        let switch_current = some_cpu && switch_current;

                        if switch_current {
                            // run current thread again.
                            cur_th.stat &= !(rt_bindings::RT_THREAD_STAT_YIELD_MASK as u8);
                            return None;
                        }

                        #[cfg(feature = "RT_USING_SMP")]
                        {
                            cur_th.oncpu = rt_bindings::RT_CPU_DETACHED as u8;
                        }

                        self.insert_thread_locked(cur_th);
                        /* consume the yield flags after scheduling */
                        cur_th.stat &= !(rt_bindings::RT_THREAD_STAT_YIELD_MASK as u8);
                    }

                    let to_th = unsafe { new_thread.as_mut() };
                    self.current_priority = highest_ready_priority as u8;
                    unsafe {
                        rt_bindings::rt_object_hook_call!(RT_SCHEDULER_HOOK, cur_th, to_th);
                    }
                    self.remove_thread_locked(to_th);
                    #[cfg(feature = "RT_USING_SMP")]
                    {
                        to_th.oncpu = cpu_id;
                    }
                    to_th.set_running();
                    // TODO: RT_SCHEDULER_STACK_CHECK
                    unsafe {
                        rt_bindings::rt_object_hook_call!(RT_SCHEDULER_SWITCH_HOOK, cur_th);
                    }
                    return Some(new_thread);
                }
                None => unreachable!(),
            }
        }
        None
    }

    pub fn start(&mut self) {
        #[cfg(feature = "RT_USING_SMP")]
        Cpus::unlock_cpus();

        let level = self.sched_lock();
        let to_thread = self.get_highest_priority_thread_locked();
        match to_thread {
            Some((mut thread, prio)) => {
                self.current_priority = prio as u8;
                let to_th = unsafe { thread.as_mut() };
                self.remove_thread_locked(to_th);
                #[cfg(feature = "RT_USING_SMP")]
                {
                    to_th.oncpu = self.get_current_id();
                }
                to_th.set_running();

                #[cfg(feature = "debug_scheduler")]
                println!(
                    "start switch to {:?}, sp {:?}",
                    to_th.get_name(),
                    to_th.stack.usage()
                );

                Cpus::arch_start();
                self.set_current_thread(thread);
                self.ctx_switch_unlock();
                // enable interrupt in context_switch_to
                Arch::context_switch_to(to_th.stack().sp());
            }
            None => panic!("!!! no thread !!!"),
        }
    }

    #[inline]
    pub fn sched_lock(&mut self) -> usize {
        // lock local first
        let level = Arch::disable_interrupts();
        self.scheduler_lock_nest.fetch_add(1, Ordering::Release);

        // lock scheduler
        #[cfg(feature = "RT_USING_SMP")]
        self.sched_lock_mp();

        level
    }

    #[inline]
    pub fn sched_unlock(&mut self, level: usize) {
        #[cfg(feature = "RT_USING_SMP")]
        self.sched_unlock_mp();

        debug_assert!(self.scheduler_lock_nest.load(Ordering::Relaxed) > 0);
        self.scheduler_lock_nest.fetch_sub(1, Ordering::Release);
        Arch::enable_interrupts(level);
    }

    #[inline]
    pub fn ctx_switch_unlock(&mut self) {
        debug_assert!(!Arch::is_interrupts_active());

        #[cfg(feature = "RT_USING_SMP")]
        self.sched_unlock_mp();

        let lock_nest = self.scheduler_lock_nest.fetch_sub(1, Ordering::Release);
        debug_assert!(lock_nest >= 1);
    }

    #[cfg(hardware_schedule)]
    pub fn sched_unlock_with_sched(&mut self, level: usize) {
        if self.scheduler_lock_nest.fetch_sub(1, Ordering::Release) == 1 {
            self.irq_switch_flag = 1;
            if likely(self.is_scheduled()) {
                Arch::trigger_switch();
            }
        }
        Arch::enable_interrupts(level);
    }

    #[cfg(not(hardware_schedule))]
    pub fn sched_unlock_with_sched(&mut self, level: usize) {
        if likely(self.is_scheduled()) {
            if Cpu::is_in_interrupt() {
                self.irq_switch_flag = 1;
                self.ctx_switch_unlock();
            } else if self.scheduler_lock_nest.load(Ordering::Relaxed) > 1 {
                self.ctx_switch_unlock();
            } else {
                // TODO: SCHED_THREAD_PREPROCESS_SIGNAL
                if let Some(to_thread) = self.prepare_context_switch_locked() {
                    unsafe {
                        let cur_thread = self.get_current_thread().unwrap_unchecked();
                        #[cfg(feature = "debug_scheduler")]
                        println!(
                            "cpu{} switch from {}: usage: {} to {}: usage: {}",
                            self.id,
                            cur_thread.as_ref().get_name(),
                            cur_thread.as_ref().stack.usage(),
                            to_thread.as_ref().get_name(),
                            to_thread.as_ref().stack.usage(),
                        );

                        #[cfg(feature = "RT_USING_OVERFLOW_CHECK")]
                        assert!(!cur_thread.as_ref().stack.check_overflow());

                        rt_bindings::rt_hw_context_switch(
                            cur_thread.as_ref().sp_ptr() as rt_bindings::rt_ubase_t,
                            to_thread.as_ref().sp_ptr() as rt_bindings::rt_ubase_t,
                            to_thread.as_ptr() as *mut rt_bindings::rt_thread,
                        );
                    }
                } else {
                    self.ctx_switch_unlock();
                }
            }
        } else {
            self.ctx_switch_unlock();
        }

        Arch::enable_interrupts(level);

        // TODO: SCHED_THREAD_PROCESS_SIGNAL
    }

    #[inline]
    pub fn is_sched_locked(&self) -> bool {
        return self.scheduler_lock_nest.load(Ordering::Relaxed) > 0;
    }

    #[inline]
    pub fn get_sched_lock_level(&self) -> u32 {
        self.scheduler_lock_nest.load(Ordering::Relaxed)
    }

    #[cfg(hardware_schedule)]
    pub fn do_task_schedule(&mut self) {
        let level = Arch::disable_interrupts();

        let lock_nest = self.scheduler_lock_nest.load(Ordering::Relaxed);
        if lock_nest > 0 {
            self.critical_switch_flag = 1;
        } else {
            self.irq_switch_flag = 1;

            #[cfg(feature = "debug_scheduler")]
            println!("trigger_switch");

            Arch::trigger_switch();
        }
        Arch::enable_interrupts(level);
    }

    #[cfg(not(hardware_schedule))]
    pub fn do_task_schedule(&mut self) {
        let level = Arch::disable_interrupts();

        if Cpu::is_in_interrupt() {
            self.irq_switch_flag = 1;
            Arch::enable_interrupts(level);
            return;
        }

        let lock_nest = self.scheduler_lock_nest.fetch_add(1, Ordering::Release);
        // TODO: add Signal preprocess.

        if lock_nest > 0 {
            self.critical_switch_flag = 1;
            self.scheduler_lock_nest.fetch_sub(1, Ordering::Release);
        } else {
            self.irq_switch_flag = 0;
            self.critical_switch_flag = 0;

            /* take the context lock before we do the real scheduling works */
            #[cfg(feature = "RT_USING_SMP")]
            self.sched_lock_mp();
            /* pick the highest runnable thread, and pass the control to it */
            match self.prepare_context_switch_locked() {
                Some(to_thread) => {
                    let cur_thread = unsafe { self.get_current_thread().unwrap_unchecked() };
                    // sched_unlock_mp will call in rt_cpus_lock_status_restore
                    unsafe {
                        #[cfg(feature = "debug_scheduler")]
                        println!(
                            "cpu{} switch from {}: usage: {} to {}: usage: {}",
                            self.id,
                            cur_thread.as_ref().get_name(),
                            cur_thread.as_ref().stack.usage(),
                            to_thread.as_ref().get_name(),
                            to_thread.as_ref().stack.usage(),
                        );

                        #[cfg(feature = "RT_USING_OVERFLOW_CHECK")]
                        assert!(!cur_thread.as_ref().stack.check_overflow());

                        rt_bindings::rt_hw_context_switch(
                            cur_thread.as_ref().sp_ptr() as rt_bindings::rt_ubase_t,
                            to_thread.as_ref().sp_ptr() as rt_bindings::rt_ubase_t,
                            to_thread.as_ptr() as *mut rt_bindings::rt_thread,
                        )
                        // cur_thread will back here
                    };
                }
                None => {
                    self.ctx_switch_unlock();
                }
            }
        }

        Arch::enable_interrupts(level);

        // TODO: add Signal process.
    }

    #[no_mangle]
    pub extern "C" fn switch_context_in_irq(stack_ptr: *mut usize) -> *mut usize {
        let scheduler = Cpu::get_current_scheduler();
        debug_assert!(scheduler.get_current_thread() != None);
        let level = Arch::disable_interrupts();

        // TODO: add Signal preprocess.

        if scheduler.irq_switch_flag == 1 {
            let lock_nest = scheduler
                .scheduler_lock_nest
                .fetch_add(1, Ordering::Release);
            if lock_nest > 0 {
                scheduler.critical_switch_flag = 1;
                scheduler
                    .scheduler_lock_nest
                    .fetch_sub(1, Ordering::Release);
            } else if !Cpu::is_in_interrupt() {
                scheduler.irq_switch_flag = 0;
                scheduler.critical_switch_flag = 0;

                #[cfg(feature = "RT_USING_SMP")]
                scheduler.sched_lock_mp();

                /* pick the highest runnable thread, and pass the control to it */
                match scheduler.prepare_context_switch_locked() {
                    Some(to_thread) => {
                        // current_thread always not None
                        let mut cur_thread =
                            unsafe { scheduler.get_current_thread().unwrap_unchecked().as_mut() };
                        #[cfg(feature = "debug_scheduler")]
                        unsafe {
                            println!(
                                "cpu{} switch from {}: usage: {} to {}: usage: {}",
                                scheduler.id,
                                cur_thread.get_name(),
                                cur_thread.stack().usage(),
                                to_thread.as_ref().get_name(),
                                to_thread.as_ref().stack().usage(),
                            );
                        }

                        #[cfg(feature = "RT_USING_OVERFLOW_CHECK")]
                        if cur_thread.stack_mut().check_overflow() {
                            panic!("stack overflow");
                        }
                        #[cfg(feature = "stack_highwater_check")]
                        if cur_thread.stack_mut().highwater() == 0 {
                            panic!("stack overflow");
                        }

                        cur_thread.stack_mut().set_sp(stack_ptr);
                        scheduler.set_current_thread(to_thread);
                        scheduler.ctx_switch_unlock();
                        return unsafe { to_thread.as_ref().stack().sp() };
                    }
                    None => {
                        scheduler.ctx_switch_unlock();
                    }
                }
            } else {
                debug_assert!(scheduler.scheduler_lock_nest.load(Ordering::Relaxed) > 0);
                scheduler
                    .scheduler_lock_nest
                    .fetch_sub(1, Ordering::Release);
            }
        }

        Arch::enable_interrupts(level);

        // not need switch, just pop.
        stack_ptr
    }

    pub(crate) fn insert_ready_locked(&mut self, thread: &mut RtThread) -> bool {
        debug_assert!(self.is_sched_locked());

        if thread.is_suspended() {
            // Quiet timeout timer first if set. ffand don't continue if we
            // failed, because it probably means that a timeout ISR racing to
            // resume thread before us.
            if thread.timer_stop() {
                // remove from suspend list
                thread.remove_tlist();
                // insert to schedule ready list and remove from susp list
                self.insert_thread_locked(thread);
                return true;
            }
        }
        false
    }

    pub(crate) fn handle_tick_increase(&mut self) {
        debug_assert!(self.get_current_thread() != None);
        let level = self.sched_lock();
        // scheduler start now, so current_thread si not None
        let thread = unsafe { self.get_current_thread().unwrap_unchecked().as_mut() };
        let need_schedule = thread.handle_tick_increase();
        if need_schedule {
            self.sched_unlock_with_sched(level);
        } else {
            self.sched_unlock(level);
        }
    }

    pub fn yield_now(&mut self) {
        debug_assert!(self.get_current_thread() != None);
        let level = self.sched_lock();
        let thread = unsafe { self.get_current_thread().unwrap_unchecked().as_mut() };
        thread.reset_to_yield();

        self.sched_unlock_with_sched(level);
    }
}

#[cfg(feature = "RT_USING_SMP")]
#[no_mangle]
pub extern "C" fn rt_scheduler_ipi_handler() {
    Cpu::get_current_scheduler().do_task_schedule();
}

#[no_mangle]
pub extern "C" fn rt_sched_lock() -> rt_bindings::rt_base_t {
    Cpu::get_current_scheduler().sched_lock() as _
}

#[no_mangle]
pub extern "C" fn rt_sched_unlock(level: rt_bindings::rt_base_t) {
    Cpu::get_current_scheduler().sched_unlock(level as usize);
}

#[no_mangle]
pub extern "C" fn rt_sched_unlock_n_resched(
    level: rt_bindings::rt_base_t,
) -> rt_bindings::rt_err_t {
    Cpu::get_current_scheduler().sched_unlock_with_sched(level as usize);
    rt_bindings::RT_EOK as rt_bindings::rt_err_t
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

// hooks
#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
#[no_mangle]
pub extern "C" fn rt_scheduler_sethook(hook: RtSchedulerHook) {
    unsafe {
        RT_SCHEDULER_HOOK = Some(hook);
    }
}

#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
#[no_mangle]
pub extern "C" fn rt_scheduler_switch_sethook(hook: RtSchedulerSwitchHook) {
    unsafe {
        RT_SCHEDULER_SWITCH_HOOK = Some(hook);
    }
}
