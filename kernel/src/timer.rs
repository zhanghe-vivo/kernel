use crate::{
    cpu::Cpu,
    object::{KObjectBase, ObjectClassType},
    static_init::UnsafeStaticInit,
    sync::RawSpin,
    thread::{Thread, ThreadWithStack},
};
use bluekernel_infra::list::doubly_linked_list::{LinkedListNode, ListHead};
use core::{
    ffi::{c_char, c_void},
    pin::Pin,
    ptr,
    ptr::addr_of_mut,
};
use pinned_init::{pin_data, pin_init, pin_init_array_from_fn, pin_init_from_closure, PinInit};

const TIMER_WHEEL_SIZE: usize = 32;

const TIMER_THREAD_PRIO: u8 = 4;

pub type TimeoutFn = extern "C" fn(*mut c_void);

const TIMER_THREAD_STACK_SIZE: usize = 2048;

pub(crate) static mut TIMER_WHEEL: UnsafeStaticInit<TimerWheel, TimerWheelInit> =
    UnsafeStaticInit::new(TimerWheelInit);

pub(crate) static mut SOFT_TIMER_WHEEL: UnsafeStaticInit<TimerWheel, TimerWheelInit> =
    UnsafeStaticInit::new(TimerWheelInit);

pub(crate) static mut TIMER_THREAD: UnsafeStaticInit<
    ThreadWithStack<TIMER_THREAD_STACK_SIZE>,
    ThreadlInit,
> = UnsafeStaticInit::new(ThreadlInit);

pub(crate) struct ThreadlInit;
unsafe impl PinInit<ThreadWithStack<TIMER_THREAD_STACK_SIZE>> for ThreadlInit {
    unsafe fn __pinned_init(
        self,
        slot: *mut ThreadWithStack<TIMER_THREAD_STACK_SIZE>,
    ) -> Result<(), core::convert::Infallible> {
        let init = ThreadWithStack::new(
            crate::c_str!("timer"),
            Timer::timer_thread_entry,
            ptr::null_mut() as *mut usize,
            TIMER_THREAD_PRIO,
            10,
        );
        unsafe { init.__pinned_init(slot) }
    }
}

pub(crate) struct TimerWheelInit;
unsafe impl PinInit<TimerWheel> for TimerWheelInit {
    unsafe fn __pinned_init(self, slot: *mut TimerWheel) -> Result<(), core::convert::Infallible> {
        let init = TimerWheel::new();
        unsafe { init.__pinned_init(slot) }
    }
}

#[derive(PartialEq)]
enum TimerStatus {
    Busy = 0,
    Idle,
}

#[derive(PartialEq)]
pub enum TimerControlAction {
    SetTime = 0,
    GetTime,
    SetOneshot,
    SetPeriodic,
    GetState,
    GetRemainTime,
    GetFuction,
    SetFunction,
    GetParm,
    SetParm,
}

impl TimerControlAction {
    pub fn from_u8(cmd: u8) -> Self {
        match cmd {
            0 => Self::SetTime,
            1 => Self::GetTime,
            2 => Self::SetOneshot,
            3 => Self::SetPeriodic,
            4 => Self::GetState,
            5 => Self::GetRemainTime,
            6 => Self::GetFuction,
            7 => Self::SetFunction,
            8 => Self::GetParm,
            _ => Self::SetParm,
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimerState(u8);

impl TimerState {
    pub const DEACTIVATED: Self = Self(0b0000_0000);
    pub const ONE_SHOT: Self = Self(0b0000_0000);
    pub const ACTIVATED: Self = Self(0b0000_0001);
    pub const PERIODIC: Self = Self(0b0000_0010);
    pub const HARD_TIMER: Self = Self(0b0000_0000);
    pub const SOFT_TIMER: Self = Self(0b0000_0100);
    pub const THREAD_TIMER: Self = Self(0b0001_0000);

    pub fn set_state(&mut self, state: Self) {
        self.0 |= state.0;
    }

    pub fn unset_state(&mut self, state: Self) {
        self.0 &= !state.0;
    }

    pub fn get_state(&self, state: Self) -> bool {
        self.0 & state.0 != 0
    }

    pub fn to_u32(self) -> u32 {
        self.0 as u32
    }
}

static mut SOFT_TIMER_STATUS: TimerStatus = TimerStatus::Idle;

#[pin_data]
pub struct TimerWheel {
    cursor: usize,
    #[pin]
    row: [ListHead; TIMER_WHEEL_SIZE],
    timer_wheel_lock: RawSpin,
}

impl TimerWheel {
    pub fn new() -> impl PinInit<Self> {
        pin_init!(Self {
            cursor: 0,
            row <- pin_init_array_from_fn(|_| ListHead::new()),
            timer_wheel_lock: RawSpin::new(),
        })
    }

    ///This function will check timer list, if a timeout event happens,
    /// the corresponding timeout function will be invoked.
    fn timer_check(&mut self, is_soft: bool) {
        self.timer_wheel_lock.lock();
        let mut cursor;
        if !is_soft {
            self.cursor = self.cursor + 1;
            cursor = self.cursor;
            if cursor == TIMER_WHEEL_SIZE {
                self.cursor = 0;
                cursor = 0;
            }
        } else {
            cursor = self.cursor;
        }
        let current_tick = Cpu::get_by_id(0).tick_load();
        crate::doubly_linked_list_for_each!(time_node, &self.row[cursor], {
            let timer = unsafe {
                &mut *crate::container_of!(time_node.as_ptr() as *mut ListHead, Timer, list_node)
            };
            if current_tick.wrapping_sub(timer.timeout_tick) < u32::MAX / 2 {
                // as timer will be removed from list, so we need to get prev
                time_node = unsafe { time_node.prev().unwrap_unchecked().as_ref() };

                timer.timer_remove();
                if !timer.flag.get_state(TimerState::PERIODIC) {
                    timer.flag.unset_state(TimerState::ACTIVATED);
                }
                if is_soft {
                    unsafe { SOFT_TIMER_STATUS = TimerStatus::Busy };
                }
                self.timer_wheel_lock.unlock();
                (timer.timeout_func)(timer.parameter);
                self.timer_wheel_lock.lock();
                unsafe {
                    if is_soft {
                        SOFT_TIMER_STATUS = TimerStatus::Idle;
                    }
                }
                if timer.flag.get_state(TimerState::PERIODIC)
                    && timer.flag.get_state(TimerState::ACTIVATED)
                {
                    timer.flag.unset_state(TimerState::ACTIVATED);
                    timer.timer_start();
                }
            } else {
                break;
            }
        });
        self.timer_wheel_lock.unlock();
    }

    ///This function will return the next timeout tick of timer wheel.
    fn next_timeout_tick(&mut self) -> u32 {
        let mut next_timeout_tick = u32::MAX;
        let _guard = self.timer_wheel_lock.acquire();
        for i in 0..TIMER_WHEEL_SIZE - 1 {
            if let Some(timer_node) = self.row[i].next() {
                unsafe {
                    let timer = crate::container_of!(timer_node.as_ptr(), Timer, list_node);
                    if (*timer).timeout_tick < next_timeout_tick {
                        next_timeout_tick = (*timer).timeout_tick;
                    }
                }
            }
        }
        next_timeout_tick
    }
}

/// The timer structure
#[repr(C)]
#[pin_data]
pub struct Timer {
    pub parent: KObjectBase,
    timeout_func: TimeoutFn,
    parameter: *mut c_void,
    pub init_tick: u32,
    pub timeout_tick: u32,
    pub flag: TimerState,
    #[pin]
    list_node: LinkedListNode,
}

crate::impl_kobject!(Timer);

impl Timer {
    /// The init funtion of the global timer
    #[inline]
    pub fn static_init(
        name: *const c_char,
        timeout_func: TimeoutFn,
        parameter: *mut c_void,
        time: u32,
        flag: u8,
    ) -> impl PinInit<Self> {
        Self::new_internal(name, timeout_func, parameter, time, flag, true)
    }

    /// The init funtion of the local timer
    #[inline]
    pub fn dyn_init(
        name: *const c_char,
        timeout_func: TimeoutFn,
        parameter: *mut c_void,
        time: u32,
        flag: u8,
    ) -> impl PinInit<Self> {
        Self::new_internal(name, timeout_func, parameter, time, flag, false)
    }

    fn new_internal(
        name: *const c_char,
        timeout_func: TimeoutFn,
        parameter: *mut c_void,
        time: u32,
        flag: u8,
        is_static: bool,
    ) -> impl PinInit<Self> {
        let init = move |slot: *mut Self| {
            let obj = unsafe { &mut *(slot as *mut KObjectBase) };
            if is_static {
                obj.init(ObjectClassType::ObjectClassTimer as u8, name);
            }
            let cur_ref = unsafe { &mut *slot };
            cur_ref.flag = TimerState(flag & !(TimerState::ACTIVATED.to_u32() as u8));
            cur_ref.init_tick = time;
            cur_ref.timeout_tick = 0;
            cur_ref.timeout_func = timeout_func;
            cur_ref.parameter = parameter;
            let _ = unsafe {
                LinkedListNode::new().__pinned_init(&mut cur_ref.list_node as *mut LinkedListNode)
            };
            Ok(())
        };
        unsafe { pin_init_from_closure(init) }
    }

    fn timer_is_timeout(&mut self) {
        self.flag.set_state(TimerState::ACTIVATED);
        if !self.flag.get_state(TimerState::PERIODIC) {
            self.flag.unset_state(TimerState::ACTIVATED);
        }
        (self.timeout_func)(self.parameter);
        if self.init_tick != 0 && self.flag.get_state(TimerState::PERIODIC) {
            let time_wheel = self.get_timer_wheel();
            let _guard = time_wheel.timer_wheel_lock.acquire();
            self.timer_start();
        }
    }

    /// This function will start the timer
    fn timer_start(&mut self) {
        let mut need_schedule = false;
        let time_wheel = self.get_timer_wheel();
        self.timer_remove();
        self.flag.unset_state(TimerState::ACTIVATED);
        let init_tick = self.init_tick;
        let timeout_tick = Cpu::get_by_id(0).tick_load() + init_tick;
        self.timeout_tick = timeout_tick;
        let cursor = (time_wheel.cursor + init_tick as usize) & (TIMER_WHEEL_SIZE - 1);
        let mut timer_node = &mut time_wheel.row[cursor];
        let header_ptr = timer_node as *mut ListHead;

        while let Some(mut next) = timer_node.next() {
            if ptr::eq(next.as_ptr(), header_ptr) {
                unsafe {
                    Pin::new_unchecked(&mut self.list_node)
                        .insert_after(Pin::new_unchecked(timer_node))
                };
                break;
            }

            timer_node = unsafe { next.as_mut() };
            let timer = unsafe { &*crate::container_of!(timer_node.as_ptr(), Timer, list_node) };
            if timeout_tick <= timer.timeout_tick {
                unsafe {
                    Pin::new_unchecked(&mut self.list_node)
                        .insert_before(Pin::new_unchecked(timer_node))
                };
                break;
            }
        }

        // timer_node is the header if next() return none
        if timer_node.next().is_none() {
            unsafe {
                Pin::new_unchecked(timer_node).push_back(Pin::new_unchecked(&mut self.list_node))
            };
        }

        self.flag.set_state(TimerState::ACTIVATED);
        if self.flag.get_state(TimerState::SOFT_TIMER) {
            unsafe {
                if SOFT_TIMER_STATUS == TimerStatus::Idle && TIMER_THREAD.stat.is_suspended() {
                    (&raw const TIMER_THREAD
                        as *const UnsafeStaticInit<ThreadWithStack<TIMER_THREAD_STACK_SIZE>, _>)
                        .cast_mut()
                        .as_mut()
                        .unwrap_unchecked()
                        .resume();
                    need_schedule = true;
                }
            }
        }
        if need_schedule {
            Cpu::get_current_scheduler().do_task_schedule();
        }
    }

    /// This function will stop the timer
    fn timer_stop(&mut self) {
        self.timer_remove();
        self.flag.unset_state(TimerState::ACTIVATED);
    }

    /// This function will remove the timer
    fn timer_remove(&mut self) {
        unsafe { Pin::new_unchecked(&mut self.list_node).remove_from_list() };
    }

    pub fn start(&mut self) {
        if self.init_tick == 0 {
            self.timer_is_timeout();
        } else {
            let time_wheel = self.get_timer_wheel();
            let _guard = time_wheel.timer_wheel_lock.acquire();
            self.timer_start();
        }
    }

    pub fn stop(&mut self) -> bool {
        let time_wheel = self.get_timer_wheel();
        let _guard = time_wheel.timer_wheel_lock.acquire();
        if !self.flag.get_state(TimerState::ACTIVATED) {
            return false;
        }
        self.timer_stop();
        true
    }

    pub fn detach(&mut self) {
        let time_wheel = self.get_timer_wheel();
        let _guard = time_wheel.timer_wheel_lock.acquire();
        self.timer_remove();
        self.flag.unset_state(TimerState::ACTIVATED);
        self.parent.detach();
    }

    pub fn delete(&mut self) {
        let time_wheel = self.get_timer_wheel();
        let _guard = time_wheel.timer_wheel_lock.acquire();
        self.timer_remove();
        self.flag.unset_state(TimerState::ACTIVATED);
        self.parent.delete();
    }

    pub fn set_timeout(&mut self, tick: u32) {
        let time_wheel = self.get_timer_wheel();
        let _guard = time_wheel.timer_wheel_lock.acquire();
        if self.flag.get_state(TimerState::ACTIVATED) {
            self.timer_remove();
            self.flag.unset_state(TimerState::ACTIVATED);
        }
        self.init_tick = tick;
    }

    pub fn restart(&mut self, tick: u32) {
        let time_wheel = self.get_timer_wheel();
        let _guard = time_wheel.timer_wheel_lock.acquire();
        if self.flag.get_state(TimerState::ACTIVATED) {
            self.timer_remove();
            self.flag.unset_state(TimerState::ACTIVATED);
        }
        self.init_tick = tick;

        if self.init_tick == 0 {
            self.timer_is_timeout();
        } else {
            self.timer_start();
        }
    }

    /// This function will get or set some options of the timer
    pub fn timer_control(&mut self, action: TimerControlAction, arg: *mut c_void) {
        let time_wheel = self.get_timer_wheel();
        let _guard = time_wheel.timer_wheel_lock.acquire();
        match action {
            TimerControlAction::GetTime => unsafe { *(arg as *mut u32) = self.init_tick },
            TimerControlAction::SetTime => {
                if self.flag.get_state(TimerState::ACTIVATED) {
                    self.timer_remove();
                    self.flag.unset_state(TimerState::ACTIVATED);
                }
                self.init_tick = unsafe { *(arg as *mut u32) };
            }
            TimerControlAction::SetOneshot => {
                self.flag.unset_state(TimerState::PERIODIC);
            }
            TimerControlAction::SetPeriodic => {
                self.flag.set_state(TimerState::PERIODIC);
            }
            TimerControlAction::GetState => {
                if self.flag.get_state(TimerState::ACTIVATED) {
                    unsafe { *(arg as *mut u32) = TimerState::ACTIVATED.to_u32() };
                } else {
                    unsafe { *(arg as *mut u32) = TimerState::DEACTIVATED.to_u32() };
                }
            }
            TimerControlAction::GetRemainTime => unsafe { *(arg as *mut u32) = self.timeout_tick },
            TimerControlAction::GetFuction => unsafe {
                *(arg as *mut TimeoutFn) = self.timeout_func
            },
            TimerControlAction::GetParm => unsafe { *(arg as *mut *mut c_void) = self.parameter },
            TimerControlAction::SetFunction => unsafe {
                self.timeout_func = *(arg as *mut TimeoutFn)
            },
            TimerControlAction::SetParm => unsafe { self.parameter = *(arg as *mut *mut c_void) },
        }
    }

    /// system timer thread entry
    extern "C" fn timer_thread_entry(_parameter: *mut c_void) {
        use crate::thread::SuspendFlag;

        let timer_wheel = unsafe { &mut *addr_of_mut!(SOFT_TIMER_WHEEL) };
        loop {
            let mut next_timeout = timer_wheel.next_timeout_tick();
            if next_timeout == u32::MAX {
                if let Some(mut thread) = crate::current_thread!() {
                    unsafe { (thread.as_mut()).suspend(SuspendFlag::Uninterruptible) };
                    Cpu::get_current_scheduler().do_task_schedule();
                };
            } else {
                next_timeout = next_timeout.wrapping_sub(Cpu::get_by_id(0).tick_load());
                if next_timeout < u32::MAX / 2 {
                    let _ = Thread::sleep(next_timeout);
                    timer_wheel.cursor =
                        (timer_wheel.cursor + next_timeout as usize) & (TIMER_WHEEL_SIZE - 1);
                }
            }
            timer_wheel.timer_check(true);
        }
    }

    /// This function will get the type of timer whell
    fn get_timer_wheel(&mut self) -> &'static mut UnsafeStaticInit<TimerWheel, TimerWheelInit> {
        let time_wheel;
        if self.flag.get_state(TimerState::SOFT_TIMER) {
            unsafe {
                time_wheel = &mut *addr_of_mut!(SOFT_TIMER_WHEEL);
            }
        } else {
            unsafe {
                time_wheel = &mut *addr_of_mut!(TIMER_WHEEL);
            }
        }
        time_wheel
    }
}

pub fn system_timer_init() {
    unsafe {
        (&raw const TIMER_WHEEL as *const UnsafeStaticInit<TimerWheel, _>)
            .as_ref()
            .unwrap_unchecked()
            .init_once();
    };
}

pub fn system_timer_thread_init() {
    unsafe {
        (&raw const TIMER_THREAD
            as *const UnsafeStaticInit<ThreadWithStack<TIMER_THREAD_STACK_SIZE>, _>)
            .as_ref()
            .unwrap_unchecked()
            .init_once();
        (&raw const SOFT_TIMER_WHEEL as *const UnsafeStaticInit<TimerWheel, _>)
            .as_ref()
            .unwrap_unchecked()
            .init_once();
        (&raw const TIMER_THREAD
            as *const UnsafeStaticInit<ThreadWithStack<TIMER_THREAD_STACK_SIZE>, _>)
            .cast_mut()
            .as_mut()
            .unwrap_unchecked()
            .start();
    }
}

pub fn timer_check() {
    assert!(Cpu::interrupt_nest_load() > 0);
    #[cfg(feature = "smp")]
    {
        if unsafe { Arch::smp::core_id() != 0u8 } {
            return;
        }
    }
    unsafe {
        (&raw const TIMER_WHEEL as *const UnsafeStaticInit<TimerWheel, _>)
            .cast_mut()
            .as_mut()
            .unwrap_unchecked()
            .timer_check(false)
    };
}
