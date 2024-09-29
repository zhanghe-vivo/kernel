use crate::{
    cpu::Cpu, linked_list::ListHead, object::*, rt_bindings, static_init::UnsafeStaticInit,
    str::CStr, sync::RawSpin, thread::RtThread, thread::ThreadWithStack,
};
use core::{ffi::c_char, ffi::c_void, pin::Pin, ptr, ptr::addr_of_mut};
use pinned_init::*;

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
            rt_bindings::RT_NULL as *mut c_void,
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
                &mut *crate::container_of!(time_node.as_ptr() as *mut ListHead, Timer, node)
            };
            if current_tick.wrapping_sub(timer.timeout_tick) < rt_bindings::RT_TICK_MAX / 2 {
                unsafe {
                    crate::rt_object_hook_call!(
                        TIMER_ENTER_HOOK,
                        &timer as *const _ as *const rt_bindings::rt_timer
                    )
                };
                timer.timer_remove();
                if (timer.parent.flag & rt_bindings::RT_TIMER_FLAG_PERIODIC as u8) == 0 {
                    timer.parent.flag &= !rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8;
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
                    crate::rt_object_hook_call!(
                        TIMER_EXIT_HOOK,
                        &timer as *const _ as *const rt_bindings::rt_timer
                    )
                };
                if ((timer.parent.flag & rt_bindings::RT_TIMER_FLAG_PERIODIC as u8) != 0)
                    && ((timer.parent.flag & rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8) != 0)
                {
                    timer.parent.flag &= !rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8;
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
        self.timer_wheel_lock.acquire();
        for i in 0..TIMER_WHEEL_SIZE - 1 {
            if let Some(timer_node) = self.row[i].next() {
                unsafe {
                    let timer = crate::container_of!(timer_node.as_ptr(), Timer, node);
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
    parent: BaseObject,
    timeout_func: TimeoutFn,
    parameter: *mut c_void,
    init_tick: u32,
    timeout_tick: u32,
    #[pin]
    node: ListHead,
}

impl Timer {
    /// The init funtion of the global timer
    #[inline]
    pub fn static_init(
        name: &'static CStr,
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
        name: &'static CStr,
        timeout_func: TimeoutFn,
        parameter: *mut c_void,
        time: u32,
        flag: u8,
    ) -> impl PinInit<Self> {
        Self::new_internal(name, timeout_func, parameter, time, flag, false)
    }

    fn new_internal(
        name: &'static CStr,
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
                    name.as_char_ptr(),
                );
            }
            let cur_ref = &mut *slot;
            cur_ref.parent.flag = flag & !rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8;
            cur_ref.init_tick = time;
            cur_ref.timeout_tick = 0;
            cur_ref.timeout_func = timeout_func;
            cur_ref.parameter = parameter;
            let _ = ListHead::new().__pinned_init(&mut cur_ref.node as *mut ListHead);
            Ok(())
        };
        unsafe { pin_init_from_closure(init) }
    }

    /// This function will start the timer
    fn timer_start(&mut self) {
        let mut is_thread_timer = false;
        let mut need_schedule = false;
        let mut level = 0;
        let time_wheel = self.get_timer_wheel();
        if self.parent.flag & rt_bindings::RT_TIMER_FLAG_THREAD_TIMER as u8 != 0 {
            is_thread_timer = true;
            level = Cpu::get_current_scheduler().sched_lock();
            let thread = unsafe {
                crate::container_of!(self as *mut Self, crate::thread::RtThread, thread_timer)
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
        self.parent.flag &= !rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8;
        unsafe {
            crate::rt_object_hook_call!(
                rt_object_take_hook,
                &self.parent as *const _ as *const rt_bindings::rt_object
            )
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
                    let timer = unsafe { &*crate::container_of!(timer_node.as_ptr(), Timer, node) };
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
        self.parent.flag |= rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8;
        if self.parent.flag & rt_bindings::RT_TIMER_FLAG_SOFT_TIMER as u8 != 0 {
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
    fn timer_stop(&mut self) {
        unsafe {
            crate::rt_object_hook_call!(
                rt_object_put_hook,
                &self.parent as *const _ as *const rt_bindings::rt_object
            )
        }
        self.timer_remove();
        self.parent.flag &= !rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8;
    }

    /// This function will remove the timer
    fn timer_remove(&mut self) {
        unsafe { Pin::new_unchecked(&mut self.node).remove() };
    }

    /// This function will get or set some options of the timer
    fn timer_control(&mut self, cmd: u32, arg: *mut c_void) {
        let time_wheel = self.get_timer_wheel();
        time_wheel.timer_wheel_lock.acquire();
        match cmd {
            rt_bindings::RT_TIMER_CTRL_GET_TIME => unsafe { *(arg as *mut u32) = self.init_tick },
            rt_bindings::RT_TIMER_CTRL_SET_TIME => {
                if (self.parent.flag & rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8) != 0 {
                    self.timer_remove();
                    self.parent.flag &= !rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8;
                }
                self.init_tick = unsafe { *(arg as *mut u32) };
            }
            rt_bindings::RT_TIMER_CTRL_SET_ONESHOT => {
                self.parent.flag &= !rt_bindings::RT_TIMER_FLAG_PERIODIC as u8;
            }
            rt_bindings::RT_TIMER_CTRL_SET_PERIODIC => {
                self.parent.flag |= rt_bindings::RT_TIMER_FLAG_PERIODIC as u8;
            }
            rt_bindings::RT_TIMER_CTRL_GET_STATE => {
                if (self.parent.flag & rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8) != 0 {
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
        if self.parent.flag & rt_bindings::RT_TIMER_FLAG_SOFT_TIMER as u8 != 0 {
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
    let name_cstr = unsafe { CStr::from_char_ptr(name) };
    let init = Timer::static_init(name_cstr, timeout, parameter, time, flag);
    unsafe {
        let _ = init.__pinned_init(timer);
    }
}

#[no_mangle]
pub extern "C" fn rt_timer_detach(timer: *mut Timer) -> rt_bindings::rt_err_t {
    assert!(!timer.is_null());
    assert!(
        rt_object_get_type(timer as rt_bindings::rt_object_t)
            == ObjectClassType::ObjectClassTimer as u8
    );
    assert!(
        rt_object_is_systemobject(timer as rt_bindings::rt_object_t)
            != rt_bindings::RT_FALSE as core::ffi::c_int
    );
    unsafe {
        let time_wheel = (*timer).get_timer_wheel();
        let _ = time_wheel.timer_wheel_lock.acquire();
        (*timer).timer_remove();
        (*timer).parent.flag &= !rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8;
    }
    rt_object_detach(unsafe { (&mut (*timer).parent) as *mut _ as *mut rt_bindings::rt_object });
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
    assert!(time < rt_bindings::RT_TICK_MAX / 2 && time > 0);
    let timer = rt_object_allocate(ObjectClassType::ObjectClassTimer as u32, name) as *mut Timer;
    if timer.is_null() {
        return ptr::null_mut();
    }
    let name_cstr = unsafe { CStr::from_char_ptr(name) };
    let init = Timer::dyn_init(name_cstr, timeout, parameter, time, flag);
    unsafe {
        let _ = init.__pinned_init(timer);
    }
    return timer;
}

#[no_mangle]
pub extern "C" fn rt_timer_delete(timer: *mut Timer) -> rt_bindings::rt_err_t {
    assert!(!timer.is_null());
    assert!(
        rt_object_get_type(timer as rt_bindings::rt_object_t)
            == ObjectClassType::ObjectClassTimer as u8
    );
    assert!(
        rt_object_is_systemobject(timer as rt_bindings::rt_object_t)
            == rt_bindings::RT_FALSE as i32
    );
    unsafe {
        let time_wheel = (*timer).get_timer_wheel();
        let _ = time_wheel.timer_wheel_lock.acquire();
        (*timer).timer_remove();
        (*timer).parent.flag &= !rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8;
    }
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
    assert!(
        rt_object_get_type(timer as rt_bindings::rt_object_t)
            == ObjectClassType::ObjectClassTimer as u8
    );
    unsafe {
        let time_wheel = (*timer).get_timer_wheel();
        let _ = time_wheel.timer_wheel_lock.acquire();
        (*timer).timer_start()
    };
    rt_bindings::RT_EOK as i32
}

#[no_mangle]
pub extern "C" fn rt_timer_stop(timer: *mut Timer) -> rt_bindings::rt_err_t {
    assert!(!timer.is_null());
    assert!(
        rt_object_get_type(timer as rt_bindings::rt_object_t)
            == ObjectClassType::ObjectClassTimer as u8
    );
    unsafe {
        let time_wheel = (*timer).get_timer_wheel();
        let _ = time_wheel.timer_wheel_lock.acquire();
        if ((*timer).parent.flag & rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8) == 0 {
            return rt_bindings::RT_ERROR as i32;
        }
        (*timer).timer_stop();
    }
    rt_bindings::RT_EOK as i32
}

#[no_mangle]
pub extern "C" fn rt_timer_control(
    timer: *mut Timer,
    cmd: core::ffi::c_int,
    arg: *mut c_void,
) -> rt_bindings::rt_err_t {
    assert!(!timer.is_null());
    assert!(
        rt_object_get_type(timer as rt_bindings::rt_object_t)
            == ObjectClassType::ObjectClassTimer as u8
    );
    unsafe { (*timer).timer_control(cmd as u32, arg) };
    rt_bindings::RT_EOK as i32
}

#[no_mangle]
pub extern "C" fn rt_timer_check() {
    assert!(Cpu::interrupt_nest_load() > 0);
    #[cfg(feature = "RT_USING_SMP")]
    {
        if unsafe { rt_bindings::rt_hw_cpu_id() != 0 } {
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
