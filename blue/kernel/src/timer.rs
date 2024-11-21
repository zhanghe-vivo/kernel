use crate::{
    cpu::Cpu, object::*, print, println, static_init::UnsafeStaticInit, sync::RawSpin,
    thread::RtThread, thread::ThreadWithStack,
};
use blue_infra::list::doubly_linked_list::ListHead;
use core::{ffi::c_char, ffi::c_void, pin::Pin, ptr, ptr::addr_of_mut};
use pinned_init::*;
use rt_bindings;

const TIMER_WHEEL_SIZE: usize = 32;

pub type TimeoutFn = extern "C" fn(*mut c_void);

#[cfg(not(feature = "RT_TIMER_THREAD_STACK_SIZE"))]
const TIMER_THREAD_STACK_SIZE: usize = 4096;
#[cfg(feature = "RT_TIMER_THREAD_STACK_SIZE")]
const TIMER_THREAD_STACK_SIZE: usize = rt_bindings::RT_TIMER_THREAD_STACK_SIZE as usize;

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
            rt_bindings::RT_NULL as *mut usize,
            rt_bindings::RT_TIMER_THREAD_PRIO as u8,
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
        crate::list_head_for_each!(time_node, &self.row[cursor], {
            let timer = unsafe {
                &mut *rt_bindings::container_of!(time_node.as_ptr() as *mut ListHead, Timer, node)
            };
            if current_tick.wrapping_sub(timer.timeout_tick) < rt_bindings::RT_TICK_MAX / 2 {
                unsafe {
                    rt_bindings::rt_object_hook_call!(
                        TIMER_ENTER_HOOK,
                        &timer as *const _ as *const rt_bindings::rt_timer
                    );
                }
                timer.timer_remove();
                if (timer.flag & rt_bindings::RT_TIMER_FLAG_PERIODIC as u8) == 0 {
                    timer.flag &= !rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8;
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
                    rt_bindings::rt_object_hook_call!(
                        TIMER_EXIT_HOOK,
                        &timer as *const _ as *const rt_bindings::rt_timer
                    );
                }
                if ((timer.flag & rt_bindings::RT_TIMER_FLAG_PERIODIC as u8) != 0)
                    && ((timer.flag & rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8) != 0)
                {
                    timer.flag &= !rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8;
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
        let mut next_timeout_tick = rt_bindings::RT_TICK_MAX;
        let _ = self.timer_wheel_lock.acquire();
        for i in 0..TIMER_WHEEL_SIZE - 1 {
            if let Some(timer_node) = self.row[i].next() {
                unsafe {
                    let timer = rt_bindings::container_of!(timer_node.as_ptr(), Timer, node);
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
    parent: KObjectBase,
    timeout_func: TimeoutFn,
    parameter: *mut c_void,
    init_tick: u32,
    timeout_tick: u32,
    flag: u8,
    #[pin]
    node: ListHead,
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
        let init = move |slot: *mut Self| unsafe {
            if is_static {
                rt_object_init(
                    slot as *mut rt_bindings::rt_object,
                    ObjectClassType::ObjectClassTimer as u32,
                    name,
                );
            }
            let cur_ref = &mut *slot;
            cur_ref.flag = flag & !rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8;
            cur_ref.init_tick = time;
            cur_ref.timeout_tick = 0;
            cur_ref.timeout_func = timeout_func;
            cur_ref.parameter = parameter;
            let _ = ListHead::new().__pinned_init(&mut cur_ref.node as *mut ListHead);
            Ok(())
        };
        unsafe { pin_init_from_closure(init) }
    }

    pub fn timer_is_timeout(&mut self) {
        self.flag |= rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8;
        if (self.flag & rt_bindings::RT_TIMER_FLAG_PERIODIC as u8) == 0 {
            self.flag &= !rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8;
        }
        (self.timeout_func)(self.parameter);
        if self.init_tick != 0 {
            let time_wheel = self.get_timer_wheel();
            let _ = time_wheel.timer_wheel_lock.acquire();
            self.timer_start();
        }
    }

    /// This function will start the timer
    pub fn timer_start(&mut self) {
        let mut is_thread_timer = false;
        let mut need_schedule = false;
        let mut level = 0;
        let time_wheel = self.get_timer_wheel();
        if self.flag & rt_bindings::RT_TIMER_FLAG_THREAD_TIMER as u8 != 0 {
            is_thread_timer = true;
            level = Cpu::get_current_scheduler().sched_lock();
            let thread = unsafe {
                rt_bindings::container_of!(self as *mut Self, crate::thread::RtThread, thread_timer)
            };
            assert!(
                rt_object_get_type(thread as rt_bindings::rt_object_t)
                    == ObjectClassType::ObjectClassThread as u8
            );
            unsafe {
                (*thread).sched_flag_ttmr_set = 1;
            }
        }
        self.timer_remove();
        self.flag &= !rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8;
        unsafe {
            rt_bindings::rt_object_hook_call!(
                rt_object_take_hook,
                &self.parent as *const _ as *const rt_bindings::rt_object
            );
        }
        let init_tick = self.init_tick;
        let timeout_tick = Cpu::get_by_id(0).tick_load() + init_tick;
        self.timeout_tick = timeout_tick;
        let cursor = (time_wheel.cursor + init_tick as usize) & (TIMER_WHEEL_SIZE - 1);
        let mut list = &mut time_wheel.row[cursor];
        let head = list.as_ptr() as *mut ListHead;
        loop {
            match list.next() {
                None => {
                    unsafe { Pin::new_unchecked(&mut self.node).insert_next(list) }
                    break;
                }
                Some(mut timer_node) => {
                    let timer =
                        unsafe { &*rt_bindings::container_of!(timer_node.as_ptr(), Timer, node) };
                    if timeout_tick <= timer.timeout_tick {
                        unsafe {
                            Pin::new_unchecked(&mut self.node).insert_prev(timer_node.as_mut())
                        };
                        break;
                    }
                    if core::ptr::eq(head, timer_node.as_ptr()) {
                        unsafe {
                            Pin::new_unchecked(&mut self.node).insert_next(timer_node.as_mut())
                        };
                        break;
                    }
                    list = unsafe { timer_node.as_mut() };
                }
            }
        }
        self.flag |= rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8;
        if self.flag & rt_bindings::RT_TIMER_FLAG_SOFT_TIMER as u8 != 0 {
            unsafe {
                if !Cpu::get_current_scheduler().is_sched_locked() {
                    level = Cpu::get_current_scheduler().sched_lock();
                }
                if SOFT_TIMER_STATUS == TimerStatus::Idle && TIMER_THREAD.is_suspended() {
                    Cpu::get_current_scheduler().sched_unlock(level);
                    TIMER_THREAD.resume();
                    need_schedule = true;
                }
            }
        }
        if is_thread_timer && Cpu::get_current_scheduler().is_sched_locked() {
            Cpu::get_current_scheduler().sched_unlock(level);
        }
        if need_schedule {
            Cpu::get_current_scheduler().do_task_schedule();
        }
    }

    /// This function will stop the timer
    pub fn timer_stop(&mut self) {
        unsafe {
            rt_bindings::rt_object_hook_call!(
                rt_object_put_hook,
                &self.parent as *const _ as *const rt_bindings::rt_object
            );
        }
        self.timer_remove();
        self.flag &= !rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8;
    }

    /// This function will remove the timer
    pub fn timer_remove(&mut self) {
        unsafe { Pin::new_unchecked(&mut self.node).remove() };
    }

    pub fn start(&mut self) {
        if self.init_tick == 0 {
            self.timer_is_timeout();
        } else {
            let time_wheel = self.get_timer_wheel();
            let _ = time_wheel.timer_wheel_lock.acquire();
            self.timer_start();
        }
    }

    pub fn stop(&mut self) -> bool {
        let time_wheel = self.get_timer_wheel();
        let _ = time_wheel.timer_wheel_lock.acquire();
        if (self.flag & rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8) == 0 {
            return false;
        }
        self.timer_stop();
        true
    }

    pub fn detach(&mut self) {
        let time_wheel = self.get_timer_wheel();
        let _ = time_wheel.timer_wheel_lock.acquire();
        self.timer_remove();
        self.flag &= !rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8;
        rt_object_detach((&mut self.parent) as *mut _ as *mut rt_bindings::rt_object);
    }

    /// This function will get or set some options of the timer
    pub fn timer_control(&mut self, cmd: u32, arg: *mut c_void) {
        let time_wheel = self.get_timer_wheel();
        let _ = time_wheel.timer_wheel_lock.acquire();
        match cmd {
            rt_bindings::RT_TIMER_CTRL_GET_TIME => unsafe { *(arg as *mut u32) = self.init_tick },
            rt_bindings::RT_TIMER_CTRL_SET_TIME => {
                if (self.flag & rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8) != 0 {
                    self.timer_remove();
                    self.flag &= !rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8;
                }
                self.init_tick = unsafe { *(arg as *mut u32) };
            }
            rt_bindings::RT_TIMER_CTRL_SET_ONESHOT => {
                self.flag &= !rt_bindings::RT_TIMER_FLAG_PERIODIC as u8;
            }
            rt_bindings::RT_TIMER_CTRL_SET_PERIODIC => {
                self.flag |= rt_bindings::RT_TIMER_FLAG_PERIODIC as u8;
            }
            rt_bindings::RT_TIMER_CTRL_GET_STATE => {
                if (self.flag & rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8) != 0 {
                    unsafe { *(arg as *mut u32) = rt_bindings::RT_TIMER_FLAG_ACTIVATED };
                } else {
                    unsafe { *(arg as *mut u32) = rt_bindings::RT_TIMER_FLAG_DEACTIVATED };
                }
            }
            rt_bindings::RT_TIMER_CTRL_GET_REMAIN_TIME => unsafe {
                *(arg as *mut u32) = self.timeout_tick
            },
            rt_bindings::RT_TIMER_CTRL_GET_FUNC => unsafe {
                *(arg as *mut TimeoutFn) = self.timeout_func
            },
            rt_bindings::RT_TIMER_CTRL_GET_PARM => unsafe {
                *(arg as *mut *mut c_void) = self.parameter
            },
            rt_bindings::RT_TIMER_CTRL_SET_FUNC => unsafe {
                self.timeout_func = *(arg as *mut TimeoutFn)
            },
            rt_bindings::RT_TIMER_CTRL_SET_PARM => unsafe {
                self.parameter = *(arg as *mut *mut c_void)
            },
            _ => {}
        }
    }

    /// system timer thread entry
    #[cfg(feature = "RT_USING_TIMER_SOFT")]
    extern "C" fn timer_thread_entry(_parameter: *mut c_void) {
        let timer_wheel = unsafe { &mut *addr_of_mut!(SOFT_TIMER_WHEEL) };
        loop {
            let mut next_timeout = timer_wheel.next_timeout_tick();
            if next_timeout == rt_bindings::RT_TICK_MAX {
                if let Some(mut thread) = crate::current_thread!() {
                    unsafe { (thread.as_mut()).suspend(rt_bindings::RT_UNINTERRUPTIBLE) };
                    Cpu::get_current_scheduler().do_task_schedule();
                };
            } else {
                next_timeout = next_timeout.wrapping_sub(Cpu::get_by_id(0).tick_load());
                if next_timeout < rt_bindings::RT_TICK_MAX / 2 {
                    let _ = RtThread::sleep(next_timeout);
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
        if self.flag & rt_bindings::RT_TIMER_FLAG_SOFT_TIMER as u8 != 0 {
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

#[no_mangle]
pub extern "C" fn rt_timer_init(
    timer: *mut Timer,
    name: *const c_char,
    timeout: TimeoutFn,
    parameter: *mut c_void,
    time: rt_bindings::rt_tick_t,
    flag: rt_bindings::rt_uint8_t,
) {
    assert!(!timer.is_null());
    assert!(!Some(timeout).is_none());
    assert!(time < rt_bindings::RT_TICK_MAX / 2);
    let init = Timer::static_init(name, timeout, parameter, time, flag);
    unsafe {
        let _ = init.__pinned_init(timer);
    }
}

#[no_mangle]
pub extern "C" fn rt_timer_detach(timer: *mut Timer) -> rt_bindings::rt_err_t {
    assert!(!timer.is_null());
    let timer_ref = unsafe { &mut *timer };
    assert!(timer_ref.type_name() == ObjectClassType::ObjectClassTimer as u8);
    assert!(timer_ref.is_static_kobject() != false);
    let time_wheel = timer_ref.get_timer_wheel();
    let _ = time_wheel.timer_wheel_lock.acquire();
    timer_ref.timer_remove();
    timer_ref.flag &= !rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8;
    rt_object_detach((&mut timer_ref.parent) as *mut _ as *mut rt_bindings::rt_object);
    rt_bindings::RT_EOK as i32
}

#[no_mangle]
pub extern "C" fn rt_timer_create(
    name: *const c_char,
    timeout: TimeoutFn,
    parameter: *mut c_void,
    time: rt_bindings::rt_tick_t,
    flag: rt_bindings::rt_uint8_t,
) -> *mut Timer {
    assert!(!Some(timeout).is_none());
    assert!(time < rt_bindings::RT_TICK_MAX / 2);
    let timer = rt_object_allocate(ObjectClassType::ObjectClassTimer as u32, name) as *mut Timer;
    if timer.is_null() {
        return ptr::null_mut();
    }
    let init = Timer::dyn_init(name, timeout, parameter, time, flag);
    unsafe {
        let _ = init.__pinned_init(timer);
    }
    return timer;
}

#[no_mangle]
pub extern "C" fn rt_timer_delete(timer: *mut Timer) -> rt_bindings::rt_err_t {
    assert!(!timer.is_null());
    let timer_ref = unsafe { &mut *timer };
    assert!(timer_ref.type_name() == ObjectClassType::ObjectClassTimer as u8);
    assert!(timer_ref.is_static_kobject() == false);
    let time_wheel = timer_ref.get_timer_wheel();
    let _ = time_wheel.timer_wheel_lock.acquire();
    timer_ref.timer_remove();
    timer_ref.flag &= !rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8;
    rt_object_delete(unsafe { (&mut (*timer).parent) as *mut _ as *mut rt_bindings::rt_object });
    rt_bindings::RT_EOK as i32
}

#[no_mangle]
pub extern "C" fn rt_system_timer_init() {
    unsafe {
        TIMER_WHEEL.init_once();
    };
}

#[no_mangle]
pub extern "C" fn rt_system_timer_thread_init() {
    unsafe {
        TIMER_THREAD.init_once();
        SOFT_TIMER_WHEEL.init_once();
        TIMER_THREAD.start();
    }
}

#[no_mangle]
pub extern "C" fn rt_timer_start(timer: *mut Timer) -> rt_bindings::rt_err_t {
    assert!(!timer.is_null());
    let timer_ref = unsafe { &mut *timer };
    assert!(timer_ref.type_name() == ObjectClassType::ObjectClassTimer as u8);
    timer_ref.start();
    rt_bindings::RT_EOK as i32
}

#[no_mangle]
pub extern "C" fn rt_timer_stop(timer: *mut Timer) -> rt_bindings::rt_err_t {
    assert!(!timer.is_null());
    let timer_ref = unsafe { &mut *timer };
    assert!(timer_ref.type_name() == ObjectClassType::ObjectClassTimer as u8);
    timer_ref.stop();
    rt_bindings::RT_EOK as i32
}

#[no_mangle]
pub extern "C" fn rt_timer_control(
    timer: *mut Timer,
    cmd: core::ffi::c_int,
    arg: *mut c_void,
) -> rt_bindings::rt_err_t {
    assert!(!timer.is_null());
    let timer_ref = unsafe { &mut *timer };
    assert!(timer_ref.type_name() == ObjectClassType::ObjectClassTimer as u8);
    timer_ref.timer_control(cmd as u32, arg);
    rt_bindings::RT_EOK as i32
}

#[no_mangle]
pub extern "C" fn rt_timer_check() {
    assert!(Cpu::interrupt_nest_load() > 0);
    #[cfg(feature = "RT_USING_SMP")]
    {
        if unsafe { blue_arch::smp::core_id() != 0u8 } {
            return;
        }
    }
    unsafe { TIMER_WHEEL.timer_check(false) };
}

#[no_mangle]
pub extern "C" fn rt_timer_next_timeout_tick() -> rt_bindings::rt_tick_t {
    unsafe { TIMER_WHEEL.next_timeout_tick() as rt_bindings::rt_tick_t }
}

#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
#[no_mangle]
pub static mut TIMER_ENTER_HOOK: Option<unsafe extern "C" fn(*const rt_bindings::rt_timer)> = None;

#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
#[no_mangle]
pub static mut TIMER_EXIT_HOOK: Option<unsafe extern "C" fn(*const rt_bindings::rt_timer)> = None;

/// This function will set a hook function on timer, which will be invoked when enter timer timeout callback function.
///
/// # Arguments
///
/// * `hook` - The hook function.
#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
#[no_mangle]
pub extern "C" fn rt_timer_enter_sethook(hook: unsafe extern "C" fn(*const rt_bindings::rt_timer)) {
    unsafe { TIMER_ENTER_HOOK = Some(hook) };
}

/// This function will set a hook function, which will be invoked when exit timer timeout callback function.
///
/// # Arguments
///
/// * `hook` - The hook function.
#[cfg(all(feature = "RT_USING_HOOK", feature = "RT_HOOK_USING_FUNC_PTR"))]
#[no_mangle]
pub extern "C" fn rt_timer_exit_sethook(hook: unsafe extern "C" fn(*const rt_bindings::rt_timer)) {
    unsafe { TIMER_EXIT_HOOK = Some(hook) };
}

#[no_mangle]
pub extern "C" fn rt_timer_info() {
    let callback_forword = || {
        println!("timer     periodic   timeout    activated     mode");
        println!("-------- ---------- ---------- ----------- ---------");
    };
    let callback = |node: &ListHead| unsafe {
        let timer = &*(crate::list_head_entry!(node.as_ptr(), KObjectBase, list) as *const Timer);
        let _ = crate::format_name!(timer.parent.name.as_ptr(), 8);
        let init_tick = timer.init_tick;
        let time_out = timer.timeout_tick;
        print!(" 0x{:08x} 0x{:08x} ", init_tick, time_out);
        if timer.flag & rt_bindings::RT_TIMER_FLAG_PERIODIC as u8 != 0 {
            print!("activated   ");
        } else {
            print!("deactivated ");
        }
        if timer.flag & rt_bindings::RT_TIMER_FLAG_PERIODIC as u8 != 0 {
            println!("periodic");
        } else {
            println!("one shot");
        }
    };
    let _ = Timer::get_info(
        callback_forword,
        callback,
        ObjectClassType::ObjectClassTimer as u8,
    );
}
