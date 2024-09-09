use crate::{
    cpu::Cpu,
    linked_list::ListHead,
    new_spinlock,
    object::*,
    println, rt_bindings,
    static_init::UnsafeStaticInit,
    str::CStr,
    sync::RawSpin,
    sync::{lock::mutex, SpinLock},
    thread::RtThread,
    thread::ThreadWithStack,
};
use alloc::ffi;
use core::{
    ffi::c_char, ffi::c_void, intrinsics::wrapping_sub, pin::Pin, ptr, sync::atomic::AtomicUsize,
    sync::atomic::Ordering,
};
use pinned_init::*;

const TIMER_WHEEL_SIZE: usize = 32;

pub type TimeoutFn = extern "C" fn(*mut c_void);

const RT_SOFT_TIMER_IDLE: u8 = 1;
const RT_SOFT_TIMER_BUSY: u8 = 0;

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

static mut SOFT_TIMER_STATUS: u8 = RT_SOFT_TIMER_IDLE;

#[pin_data]
struct TimerWheel {
    current_ptr: AtomicUsize,
    #[pin]
    row: [ListHead; TIMER_WHEEL_SIZE],
    timer_wheel_lock: RawSpin,
}

impl TimerWheel {
    pub fn new() -> impl PinInit<Self> {
        pin_init!(Self {
            current_ptr: AtomicUsize::new(0),
            row <- pin_init_array_from_fn(|_| ListHead::new()),
            timer_wheel_lock: RawSpin::new(),
        })
    }

    fn timer_check(&mut self) {
        assert!(Cpu::interrupt_nest_load() > 0);
        #[cfg(feature = "RT_USING_SMP")]
        {
            if unsafe { rt_bindings::rt_hw_cpu_id() != 0 } {
                return;
            }
        }
        self.current_ptr.fetch_add(1, Ordering::SeqCst);
        let mut ptr = self.current_ptr.load(Ordering::SeqCst);
        if ptr == TIMER_WHEEL_SIZE {
            self.current_ptr.store(0, Ordering::SeqCst);
            ptr = 0;
        }
        let current_tick = Cpu::get_by_id(0).tick_load();
        self.timer_wheel_lock.lock();
        crate::list_head_for_each!(time_node, &self.row[ptr], {
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
                self.timer_wheel_lock.unlock();
                timer.timer_remove();
                if (timer.parent.flag & rt_bindings::RT_TIMER_FLAG_PERIODIC as u8) == 0 {
                    timer.parent.flag &= !rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8;
                }
                (timer.timeout_func)(timer.parameter);
                self.timer_wheel_lock.lock();
                unsafe {
                    crate::rt_object_hook_call!(
                        TIMER_EXIT_HOOK,
                        &timer as *const _ as *const rt_bindings::rt_timer
                    )
                };
                if ((timer.parent.flag & rt_bindings::RT_TIMER_FLAG_PERIODIC as u8) != 0)
                    && ((timer.parent.flag & rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8) != 0)
                {
                    timer.parent.flag &= !rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8;
                    self.timer_wheel_lock.unlock();
                    timer.timer_start();
                    self.timer_wheel_lock.lock();
                }
            } else {
                break;
            }
        });
        self.timer_wheel_lock.unlock();
    }

    #[cfg(feature = "RT_USING_TIMER_SOFT")]
    fn soft_timer_check(&mut self) {
        let ptr = self.current_ptr.load(Ordering::SeqCst);
        let current_tick = Cpu::get_by_id(0).tick_load();
        println!("soft_timer_check");
        self.timer_wheel_lock.lock();
        crate::list_head_for_each!(time_node, &self.row[ptr], {
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
                self.timer_wheel_lock.unlock();
                timer.timer_remove();
                if (timer.parent.flag & rt_bindings::RT_TIMER_FLAG_PERIODIC as u8) == 0 {
                    timer.parent.flag &= !rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8;
                }
                (timer.timeout_func)(timer.parameter);
                self.timer_wheel_lock.lock();
                unsafe { SOFT_TIMER_STATUS = RT_SOFT_TIMER_BUSY };
                unsafe {
                    SOFT_TIMER_STATUS = RT_SOFT_TIMER_IDLE;
                    crate::rt_object_hook_call!(
                        TIMER_EXIT_HOOK,
                        &timer as *const _ as *const rt_bindings::rt_timer
                    );
                }
                if ((timer.parent.flag & rt_bindings::RT_TIMER_FLAG_PERIODIC as u8) != 0)
                    && ((timer.parent.flag & rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8) != 0)
                {
                    timer.parent.flag &= !rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8;
                    self.timer_wheel_lock.unlock();
                    timer.timer_start();
                    self.timer_wheel_lock.lock();
                }
            } else {
                break;
            }
        });
        self.timer_wheel_lock.unlock();
    }

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
        println!("next timeout {}", next_timeout_tick);
        next_timeout_tick
    }
}

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
            } else {
                rt_object_init_dyn(
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

    fn timer_start(&mut self) {
        let time_wheel;
        let mut is_thread_timer = false;
        let mut need_schedule = false;
        let mut level = 0;
        if self.parent.flag & rt_bindings::RT_TIMER_FLAG_SOFT_TIMER as u8 != 0 {
            time_wheel = unsafe { &mut SOFT_TIMER_WHEEL };
        } else {
            time_wheel = unsafe { &mut TIMER_WHEEL };
        }

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
        time_wheel.timer_wheel_lock.acquire();
        self.parent.flag &= !rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8;
        unsafe {
            crate::rt_object_hook_call!(
                rt_object_take_hook,
                &self.parent as *const _ as *const rt_bindings::rt_object
            )
        }
        let timeout_tick = Cpu::get_by_id(0).tick_load() + self.init_tick;
        self.timeout_tick = timeout_tick;
        let index = self.init_tick as usize & (TIMER_WHEEL_SIZE - 1);
        let mut insert_ptr = time_wheel.current_ptr.load(Ordering::SeqCst) + index;
        if insert_ptr >= TIMER_WHEEL_SIZE {
            insert_ptr = insert_ptr & (TIMER_WHEEL_SIZE - 1);
        }
        if self.parent.flag & rt_bindings::RT_TIMER_FLAG_SOFT_TIMER as u8 != 0 {
            println!("insert_ptr = {}", insert_ptr);
        }
        let mut list = &mut time_wheel.row[insert_ptr];
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
                if SOFT_TIMER_STATUS == RT_SOFT_TIMER_IDLE
                    && ((TIMER_THREAD.stat & rt_bindings::RT_THREAD_SUSPEND_MASK as u8)
                        == rt_bindings::RT_THREAD_SUSPEND_MASK as u8)
                {
                    TIMER_THREAD.resume();
                    need_schedule = true;
                }
            }
        }
        if is_thread_timer {
            Cpu::get_current_scheduler().sched_unlock(level);
        }
        if need_schedule {
            Cpu::get_current_scheduler().do_task_schedule();
        }
    }

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

    fn timer_remove(&mut self) {
        let time_wheel;
        if self.parent.flag & rt_bindings::RT_TIMER_FLAG_SOFT_TIMER as u8 != 0 {
            time_wheel = unsafe { &mut SOFT_TIMER_WHEEL };
        } else {
            time_wheel = unsafe { &mut TIMER_WHEEL };
        }
        let _ = time_wheel.timer_wheel_lock.acquire();
        unsafe { Pin::new_unchecked(&mut self.node).remove() };
        self.parent.flag &= !rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8;
    }

    fn timer_control(&mut self, cmd: u32, arg: *mut c_void) {
        let time_wheel;
        if self.parent.flag & rt_bindings::RT_TIMER_FLAG_SOFT_TIMER as u8 != 0 {
            time_wheel = unsafe { &mut SOFT_TIMER_WHEEL };
        } else {
            time_wheel = unsafe { &mut TIMER_WHEEL };
        }
        time_wheel.timer_wheel_lock.lock();
        match cmd {
            rt_bindings::RT_TIMER_CTRL_GET_TIME => unsafe { *(arg as *mut u32) = self.init_tick },
            rt_bindings::RT_TIMER_CTRL_SET_TIME => {
                if (self.parent.flag & rt_bindings::RT_TIMER_FLAG_ACTIVATED as u8) != 0 {
                    time_wheel.timer_wheel_lock.unlock();
                    self.timer_remove();
                    time_wheel.timer_wheel_lock.lock();
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
        time_wheel.timer_wheel_lock.unlock();
    }

    #[cfg(feature = "RT_USING_TIMER_SOFT")]
    extern "C" fn timer_thread_entry(_parameter: *mut c_void) {
        let timer_wheel = unsafe { &mut SOFT_TIMER_WHEEL };
        loop {
            let mut next_timeout = timer_wheel.next_timeout_tick();
            if next_timeout == rt_bindings::RT_TICK_MAX {
                let thread = match Cpu::get_current_thread() {
                    Some(thread) => thread.as_ptr(),
                    None => ptr::null_mut(),
                };
                assert!(!thread.is_null());
                assert!(
                    rt_object_get_type(thread as rt_bindings::rt_object_t)
                        == ObjectClassType::ObjectClassThread as u8
                );
                unsafe { (&mut *thread).suspend(rt_bindings::RT_UNINTERRUPTIBLE) };
                Cpu::get_current_scheduler().do_task_schedule();
            } else {
                if next_timeout.wrapping_sub(Cpu::get_by_id(0).tick_load())
                    < rt_bindings::RT_TICK_MAX / 2
                {
                    next_timeout = next_timeout - Cpu::get_by_id(0).tick_load();
                    RtThread::sleep(next_timeout);
                    let index = next_timeout as usize & (TIMER_WHEEL_SIZE - 1);
                    timer_wheel.current_ptr.fetch_add(index, Ordering::SeqCst);
                    let ptr = timer_wheel.current_ptr.load(Ordering::SeqCst);
                    if ptr >= TIMER_WHEEL_SIZE {
                        let new_ptr = ptr & (TIMER_WHEEL_SIZE - 1);
                        timer_wheel.current_ptr.store(new_ptr, Ordering::SeqCst);
                    }
                    println!(
                        "current_ptr = {}",
                        timer_wheel.current_ptr.load(Ordering::SeqCst)
                    );
                }
            }
            timer_wheel.soft_timer_check()
        }
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
        let init = init.__pinned_init(timer);
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
        (*timer).timer_remove();
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
    assert!(time < rt_bindings::RT_TICK_MAX / 2);
    let timer = rt_object_allocate(ObjectClassType::ObjectClassTimer as u32, name) as *mut Timer;
    if timer.is_null() {
        return ptr::null_mut();
    }
    let name_cstr = unsafe { CStr::from_char_ptr(name) };
    let init = Timer::dyn_init(name_cstr, timeout, parameter, time, flag);
    unsafe {
        let init = init.__pinned_init(timer);
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
        (*timer).timer_remove();
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
    unsafe { (*timer).timer_start() };
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
    unsafe { TIMER_WHEEL.timer_check() };
}

#[no_mangle]
pub extern "C" fn rt_timer_next_timeout_tick() -> rt_bindings::rt_tick_t {
    println!("rt_timer_next_timeout_tick");
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
