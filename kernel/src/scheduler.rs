#[cfg(feature = "debugging_scheduler")]
use crate::println;
use crate::{
    arch::Arch,
    bluekernel_kconfig::THREAD_PRIORITY_MAX,
    cpu::Cpu,
    thread::{Thread, ThreadState},
};
#[cfg(feature = "smp")]
use crate::{
    cpu::{Cpus, CPUS_NR, CPUS_NUMBER},
    sync::RawSpin,
};
use bluekernel_infra::list::doubly_linked_list::ListHead;
use core::{
    intrinsics::likely,
    pin::Pin,
    ptr::{self, NonNull},
    sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, Ordering},
};
use pinned_init::{pin_data, pin_init, pin_init_array_from_fn, PinInit};

struct DisableInterruptGuard {
    level: usize,
}

impl DisableInterruptGuard {
    pub fn new() -> Self {
        Self {
            level: Arch::disable_interrupts(),
        }
    }
}

impl Drop for DisableInterruptGuard {
    fn drop(&mut self) {
        Arch::enable_interrupts(self.level);
    }
}

#[cfg(feature = "smp")]
const CPU_MASK: usize = (1 << CPUS_NR) - 1;

#[cfg(feature = "thread_priority_max")]
#[pin_data]
pub struct PriorityTableManager {
    #[pin]
    priority_table: [ListHead; THREAD_PRIORITY_MAX as usize],
    priority_group: u32,
    ready_table: [u8; 32],
}

#[cfg(not(feature = "thread_priority_max"))]
#[pin_data]
pub struct PriorityTableManager {
    #[pin]
    priority_table: [ListHead; THREAD_PRIORITY_MAX as usize],
    priority_group: u32,
}

#[pin_data]
pub struct Scheduler {
    id: u8,
    pub(crate) current_thread: AtomicPtr<Thread>,
    // Scheduler lock as local irq, not need spin_lock
    ///priority list headers
    #[pin]
    priority_manager: PriorityTableManager,
    current_priority: u8,
    preempt_count: AtomicU32,
    need_resched: AtomicBool,
}

impl PriorityTableManager {
    #[cfg(feature = "thread_priority_max")]
    pub fn new() -> impl PinInit<Self> {
        pin_init!(Self {
            priority_table <- pin_init_array_from_fn(|_| ListHead::new()),
            priority_group: 0,
            ready_table: [0; 32],
        })
    }

    #[cfg(not(feature = "thread_priority_max"))]
    pub fn new() -> impl PinInit<Self> {
        pin_init!(Self {
            priority_table <- pin_init_array_from_fn(|_| ListHead::new()),
            priority_group: 0,
        })
    }

    #[inline]
    pub fn get_priority_group(&self) -> u32 {
        self.priority_group
    }

    #[inline]
    pub fn get_highest_ready_prio(&self) -> u32 {
        let num = self.priority_group.trailing_zeros();
        #[cfg(feature = "thread_priority_max")]
        if num != u32::MAX {
            return (num << 3) + self.ready_table[num as usize].trailing_zeros();
        }

        num
    }

    pub fn get_thread_by_prio(&self, prio: u32) -> Option<NonNull<Thread>> {
        if let Some(node) = self.priority_table[prio as usize].next() {
            unsafe {
                let thread: *mut Thread = crate::thread_list_node_entry!(node.as_ptr());
                return Some(NonNull::new_unchecked(thread));
            }
        }
        None
    }

    pub fn insert_thread(&mut self, thread: &mut Thread) {
        debug_assert!(thread.list_node.is_empty());
        #[cfg(feature = "thread_priority_max")]
        {
            self.ready_table[thread.priority.get_number() as usize] |=
                thread.priority.get_high_mask() as u8;
        }
        self.priority_group |= thread.priority.get_number_mask();

        // There is no time slices left(YIELD), inserting thread before ready list.
        if thread.stat.is_yield() {
            unsafe {
                Pin::new_unchecked(
                    &mut self.priority_table[thread.priority.get_current() as usize],
                )
                .push_back(Pin::new_unchecked(&mut thread.list_node));
            }
        } else {
            unsafe {
                Pin::new_unchecked(
                    &mut self.priority_table[thread.priority.get_current() as usize],
                )
                .push_front(Pin::new_unchecked(&mut thread.list_node));
            }
        }
    }

    pub fn remove_thread(&mut self, thread: &mut Thread) {
        unsafe { Pin::new_unchecked(&mut thread.list_node).remove_from_list() };

        if self.priority_table[thread.priority.get_current() as usize].is_empty() {
            #[cfg(feature = "thread_priority_max")]
            {
                self.ready_table[thread.priority.get_number() as usize] &=
                    !thread.priority.get_high_mask();
                if self.ready_table[thread.priority.get_number() as usize] == 0 {
                    self.priority_group &= !(thread.priority.get_number_mask());
                }
            }
            #[cfg(not(feature = "thread_priority_max"))]
            {
                self.priority_group &= !(thread.priority.get_number_mask());
            }
        }
    }
}

impl Scheduler {
    pub fn new(index: u8) -> impl PinInit<Self> {
        pin_init!(Self {
            current_thread: AtomicPtr::new(ptr::null_mut()),
            priority_manager <- PriorityTableManager::new(),
            preempt_count: AtomicU32::new(0),
            id: index,
            current_priority: (THREAD_PRIORITY_MAX - 1) as u8,
            need_resched: AtomicBool::new(false),
        })
    }

    #[inline]
    pub const fn get_current_id(&self) -> u8 {
        self.id
    }

    #[inline]
    pub fn get_current_thread(&self) -> Option<NonNull<Thread>> {
        NonNull::new(self.current_thread.load(Ordering::Acquire))
    }

    #[inline]
    pub fn set_current_thread(&self, th: NonNull<Thread>) {
        self.current_thread.store(th.as_ptr(), Ordering::Release);
    }

    #[inline]
    pub fn is_scheduled(&self) -> bool {
        // This method might be used as predicate in the context, so `Acquire` is required.
        self.current_thread.load(Ordering::Acquire) != ptr::null_mut()
    }

    #[inline]
    pub fn need_reschedule(&self) -> bool {
        self.need_resched.load(Ordering::Acquire)
    }

    #[inline]
    pub fn set_need_reschedule(&mut self, val: bool) {
        self.need_resched.store(val, Ordering::Release)
    }

    #[inline]
    pub fn preemptable(&self) -> bool {
        self.preempt_count.load(Ordering::Acquire) == 0
    }

    #[inline]
    pub fn preempt_disable(&self) -> bool {
        return self.preempt_count.fetch_add(1, Ordering::AcqRel) == 0;
    }

    #[inline]
    pub fn preempt_enable(&mut self) -> bool {
        // FIXME: Why do we need to disable interrupt here?
        let _ = DisableInterruptGuard::new();
        if self.preempt_enable_no_resched() {
            if self.need_reschedule() {
                self.do_task_schedule();
            }
            return true;
        }
        return false;
    }

    #[inline]
    pub fn preempt_enable_no_resched(&mut self) -> bool {
        debug_assert!(self.preempt_count.load(Ordering::Acquire) > 0);
        self.preempt_count.fetch_sub(1, Ordering::AcqRel) == 1
    }

    #[cfg(feature = "smp")]
    #[inline]
    fn sched_lock_smp(&mut self) {
        Cpus::lock_sched_fast();
    }

    #[cfg(feature = "smp")]
    #[inline]
    fn sched_unlock_smp(&mut self) {
        Cpus::unlock_sched_fast();
    }

    fn get_highest_priority_thread_locked(&self) -> Option<(NonNull<Thread>, u32)> {
        debug_assert!(self.is_sched_locked());

        let highest = self.priority_manager.get_highest_ready_prio();

        #[cfg(feature = "smp")]
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

    pub fn insert_thread_locked(&mut self, thread: &mut Thread) {
        debug_assert!(self.is_sched_locked());

        if thread.stat.is_ready() {
            return;
        }

        #[cfg(feature = "smp")]
        if !thread.is_cpu_detached() {
            // only YIELD -> READY, SUSPEND -> READY is allowed by this API. However,
            // this is a RUNNING thread. So here we reset it's status and let it go.
            thread.stat.set_base_state(ThreadState::RUNNING);
            return;
        }

        thread.stat.set_base_state(ThreadState::READY);

        #[cfg(not(feature = "smp"))]
        self.priority_manager.insert_thread(thread);

        #[cfg(feature = "smp")]
        {
            let cpu_id = self.get_current_id();
            let bind_cpu = thread.get_bind_cpu();
            if bind_cpu == CPUS_NUMBER as u8 {
                Cpus::insert_thread_to_global(thread);
            } else {
                if bind_cpu == cpu_id {
                    self.priority_manager.insert_thread(thread);
                } else {
                    Cpu::get_scheduler_by_id(bind_cpu)
                        .priority_manager
                        .insert_thread(thread);
                }
            }
        }
    }

    pub fn remove_thread_locked(&mut self, thread: &mut Thread) {
        debug_assert!(self.is_sched_locked());

        #[cfg(not(feature = "smp"))]
        self.priority_manager.remove_thread(thread);

        #[cfg(feature = "smp")]
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

    pub fn change_priority_locked(&mut self, thread: &mut Thread, priority: u8) {
        debug_assert!(self.is_scheduled());
        debug_assert!(self.is_sched_locked());
        assert!(priority < THREAD_PRIORITY_MAX as u8);

        if thread.stat.is_ready() {
            self.remove_thread_locked(thread);
            thread.priority.update(priority);
            thread.stat.set_base_state(ThreadState::INIT);
            self.insert_thread_locked(thread);
        } else {
            thread.priority.update(priority);
        }
    }

    #[inline]
    fn has_ready_thread(&self) -> bool {
        #[cfg(not(feature = "smp"))]
        return self.priority_manager.priority_group != 0;

        #[cfg(feature = "smp")]
        return Cpus::get_priority_group_from_global() != 0
            || self.priority_manager.priority_group != 0;
    }

    fn prepare_context_switch_locked(
        &mut self,
        cur_th: Option<NonNull<Thread>>,
    ) -> Option<NonNull<Thread>> {
        // Quickly check if any other ready threads queuing.
        if self.has_ready_thread() {
            let to_thread = self.get_highest_priority_thread_locked();
            match to_thread {
                Some((mut new_thread, highest_ready_priority)) => {
                    if let Some(mut cur_th) = cur_th {
                        #[cfg(feature = "smp")]
                        let cpu_id = self.get_current_id();

                        let cur_th = unsafe { cur_th.as_mut() };
                        // Check if current thread can be running on current core again.
                        if cur_th.stat.is_running() {
                            let switch_current = cur_th.priority.get_current()
                                < highest_ready_priority as u8
                                || (cur_th.priority.get_current() == highest_ready_priority as u8
                                    && !cur_th.stat.is_yield());
                            #[cfg(feature = "smp")]
                            let some_cpu = cur_th.get_bind_cpu() == CPUS_NUMBER as u8
                                || cur_th.get_bind_cpu() == cpu_id;
                            #[cfg(feature = "smp")]
                            let switch_current = some_cpu && switch_current;

                            if switch_current {
                                // Run current thread again.
                                cur_th.stat.clear_yield();
                                return None;
                            }

                            #[cfg(feature = "smp")]
                            {
                                cur_th.oncpu = CPU_DETACHED as u8;
                            }

                            self.insert_thread_locked(cur_th);
                            // Consume the yield flags after scheduling.
                            cur_th.stat.clear_yield();
                        }
                    }

                    let to_th = unsafe { new_thread.as_mut() };
                    self.current_priority = highest_ready_priority as u8;

                    self.remove_thread_locked(to_th);
                    #[cfg(feature = "smp")]
                    {
                        to_th.oncpu = cpu_id;
                    }
                    to_th.stat.set_base_state(ThreadState::RUNNING);

                    return Some(new_thread);
                }
                None => unreachable!(),
            }
        }
        None
    }

    #[cfg(hardware_schedule)]
    pub fn start(&mut self) {
        self.set_need_reschedule(true);
        Arch::start_switch();
    }

    #[cfg(not(hardware_schedule))]
    pub fn start(&mut self) {
        #[cfg(feature = "smp")]
        Cpus::unlock_cpus();

        let level = self.sched_lock();
        let to_thread = self.get_highest_priority_thread_locked();
        match to_thread {
            Some((mut thread, prio)) => {
                self.current_priority = prio as u8;
                let to_th = unsafe { thread.as_mut() };
                self.remove_thread_locked(to_th);
                #[cfg(feature = "smp")]
                {
                    to_th.oncpu = self.get_current_id();
                }
                to_th.stat.set_base_state(ThreadState::RUNNING);

                #[cfg(feature = "debugging_scheduler")]
                println!(
                    "Switching to {:?}, usage of stack: {:?}",
                    (to_th as *const Thread),
                    to_th.stack.usage()
                );

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
        self.preempt_disable();

        // lock scheduler
        #[cfg(feature = "smp")]
        self.sched_lock_smp();

        level
    }

    #[inline]
    pub fn sched_unlock(&mut self, level: usize) {
        #[cfg(feature = "smp")]
        self.sched_unlock_smp();

        debug_assert!(!self.preemptable());
        self.preempt_enable_no_resched();
        Arch::enable_interrupts(level);
    }

    #[inline]
    pub fn ctx_switch_unlock(&mut self) {
        debug_assert!(!Arch::is_interrupts_active());

        #[cfg(feature = "smp")]
        self.sched_unlock_smp();

        self.preempt_enable_no_resched();
    }

    #[cfg(hardware_schedule)]
    pub fn sched_unlock_with_sched(&mut self, level: usize) {
        if self.preempt_enable_no_resched() {
            self.set_need_reschedule(true);
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
                self.set_need_reschedule(true);
                self.ctx_switch_unlock();
            } else if !self.preemptable() {
                self.ctx_switch_unlock();
            } else {
                let cur_thread = self.get_current_thread();
                if let Some(to_thread) = self.prepare_context_switch_locked(cur_thread) {
                    unsafe {
                        #[cfg(feature = "debugging_scheduler")]
                        println!(
                            "cpu #{} switches from {:?} (stack usage: {}) to {:?} (stack usage: {})",
                            self.id,
                            cur_thread.unwrap().as_ptr(),
                            cur_thread.unwrap().as_ref().stack.usage(),
                            to_thread.as_ptr(),
                            to_thread.as_ref().stack.usage(),
                        );

                        #[cfg(feature = "overflow_check")]
                        assert!(!cur_thread.as_ref().stack.check_overflow());
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
        return !self.preemptable();
    }

    #[inline]
    pub fn get_sched_lock_level(&self) -> u32 {
        self.preempt_count.load(Ordering::Acquire)
    }

    #[cfg(hardware_schedule)]
    pub fn do_task_schedule(&mut self) {
        if !self.is_scheduled() {
            return;
        }
        let _ = DisableInterruptGuard::new();
        self.set_need_reschedule(true);
        if self.preemptable() {
            #[cfg(feature = "debugging_scheduler")]
            println!("Scheduler is triggering context switch...");
            Arch::trigger_switch();
        }
    }

    #[cfg(not(hardware_schedule))]
    pub fn do_task_schedule(&mut self) {
        if !self.is_scheduled() {
            return;
        }
        let _ = DisableInterruptGuard::new();
        if Cpu::is_in_interrupt() {
            self.set_need_reschedule(true);
            return;
        }
        let changed = self.preempt_disable();
        if !changed {
            self.set_need_reschedule(true);
            self.preempt_enable_no_resched();
        } else {
            self.set_need_reschedule(false);
            // Take the context lock before we do the real scheduling works.
            #[cfg(feature = "smp")]
            self.sched_lock_smp();
            // Pick the highest runnable thread, and pass the control to it.
            let cur_thread = self.get_current_thread();
            if let Some(to_thread) = self.prepare_context_switch_locked(cur_thread) {
                unsafe {
                    #[cfg(feature = "debugging_scheduler")]
                    println!(
                        "cpu #{} switches from {:?} (stack usage: {}) to {:?} (stack usage: {})",
                        self.id,
                        cur_thread.unwrap().as_ptr(),
                        cur_thread.unwrap().as_ref().stack.usage(),
                        to_thread.as_ptr(),
                        to_thread.as_ref().stack.usage(),
                    );

                    #[cfg(feature = "overflow_check")]
                    assert!(!cur_thread.as_ref().stack.check_overflow());
                }
            } else {
                self.ctx_switch_unlock();
            }
        }

        // TODO: add Signal process.
    }

    #[no_mangle]
    pub extern "C" fn switch_context_in_irq(stack_ptr: *mut usize) -> *mut usize {
        let _ = DisableInterruptGuard::new();
        let scheduler = Cpu::get_current_scheduler();
        // TODO: add Signal preprocess.
        if scheduler.need_reschedule() {
            let changed = scheduler.preempt_disable();
            if !changed {
                scheduler.set_need_reschedule(true);
                scheduler.preempt_enable_no_resched();
            } else if !Cpu::is_in_interrupt() {
                scheduler.set_need_reschedule(false);

                #[cfg(feature = "smp")]
                scheduler.sched_lock_smp();

                // Pick the highest runnable thread, and pass the control to it.
                let cur_thread = scheduler.get_current_thread();
                if let Some(to_thread) = scheduler.prepare_context_switch_locked(cur_thread) {
                    if let Some(mut cur_th) = cur_thread {
                        #[cfg(feature = "debugging_scheduler")]
                        unsafe {
                            println!(
																"cpu #{} switches from {:?} (stack usage: {}) to {:?} (stack usage: {})",
                                scheduler.id,
                                cur_th.as_ptr(),
                                cur_th.as_ref().stack().usage(),
                                to_thread.as_ptr(),
                                to_thread.as_ref().stack().usage(),
                            );
                        }
                        #[cfg(feature = "overflow_check")]
                        unsafe {
                            if cur_th.as_mut().stack_mut().check_overflow() {
                                panic!("stack overflow");
                            }
                        }
                        #[cfg(feature = "stack_highwater_check")]
                        unsafe {
                            if cur_th.as_mut().stack_mut().highwater() == 0 {
                                panic!("stack overflow");
                            }
                        }
                        unsafe { cur_th.as_mut().stack_mut().set_sp(stack_ptr) };
                    } else {
                        #[cfg(feature = "debugging_scheduler")]
                        unsafe {
                            println!(
                                "cpu #{} switches to {:?} (stack usage: {})",
                                scheduler.id,
                                to_thread.as_ptr(),
                                to_thread.as_ref().stack().usage(),
                            );
                        }
                    }
                    scheduler.set_current_thread(to_thread);
                    scheduler.ctx_switch_unlock();
                    return unsafe { to_thread.as_ref().stack().sp() };
                } else {
                    scheduler.ctx_switch_unlock();
                }
            } else {
                debug_assert!(!scheduler.preemptable());
                scheduler.preempt_enable_no_resched();
            }
        }

        // No need to switch, just pop.
        stack_ptr
    }

    pub(crate) fn insert_ready_locked(&mut self, thread: &mut Thread) -> bool {
        debug_assert!(self.is_sched_locked());
        debug_assert!(thread.list_node.is_empty());
        if !thread.stat.is_suspended() {
            return false;
        }
        // Stop thread timer anyway.
        thread.timer_stop();
        // Insert to schedule ready list and remove from susp list.
        self.insert_thread_locked(thread);
        return true;
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
