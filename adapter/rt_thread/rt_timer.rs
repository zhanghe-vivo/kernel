use crate::bluekernel::{
    error::code,
    object::{KObjectBase, KernelObject, ObjectClassType},
    timer::{TimeoutFn, Timer, TimerControlAction},
};
use core::{ffi, ptr};
use pinned_init::PinInit;

#[no_mangle]
pub extern "C" fn rt_timer_init(
    timer: *mut Timer,
    name: *const ffi::c_char,
    timeout: TimeoutFn,
    parameter: *mut ffi::c_void,
    time: u32,
    flag: u8,
) {
    assert!(!timer.is_null());
    assert!(!Some(timeout).is_none());
    assert!(time < u32::MAX);
    let init = Timer::static_init(name, timeout, parameter, time, flag);
    unsafe {
        let _ = init.__pinned_init(timer);
    }
}

#[no_mangle]
pub extern "C" fn rt_timer_detach(timer: *mut Timer) -> i32 {
    assert!(!timer.is_null());
    let timer_ref = unsafe { &mut *timer };
    assert!(timer_ref.type_name() == ObjectClassType::ObjectClassTimer as u8);
    assert!(timer_ref.is_static_kobject() != false);
    timer_ref.detach();
    code::EOK.to_errno()
}

#[no_mangle]
pub extern "C" fn rt_timer_create(
    name: *const ffi::c_char,
    timeout: TimeoutFn,
    parameter: *mut ffi::c_void,
    time: u32,
    flag: u8,
) -> *mut Timer {
    assert!(!Some(timeout).is_none());
    assert!(time < u32::MAX);
    let timer = KObjectBase::new_raw(ObjectClassType::ObjectClassTimer as u8, name) as *mut Timer;
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
pub extern "C" fn rt_timer_delete(timer: *mut Timer) -> i32 {
    assert!(!timer.is_null());
    let timer_ref = unsafe { &mut *timer };
    assert!(timer_ref.type_name() == ObjectClassType::ObjectClassTimer as u8);
    assert!(!timer_ref.is_static_kobject());
    timer_ref.delete();
    code::EOK.to_errno()
}

#[no_mangle]
pub extern "C" fn rt_timer_start(timer: *mut Timer) -> i32 {
    assert!(!timer.is_null());
    let timer_ref = unsafe { &mut *timer };
    assert!(timer_ref.type_name() == ObjectClassType::ObjectClassTimer as u8);
    timer_ref.start();
    code::EOK.to_errno()
}

#[no_mangle]
pub extern "C" fn rt_timer_stop(timer: *mut Timer) -> i32 {
    assert!(!timer.is_null());
    let timer_ref = unsafe { &mut *timer };
    assert!(timer_ref.type_name() == ObjectClassType::ObjectClassTimer as u8);
    timer_ref.stop();
    code::EOK.to_errno()
}

#[no_mangle]
pub extern "C" fn rt_timer_control(timer: *mut Timer, cmd: u8, arg: *mut ffi::c_void) -> i32 {
    assert!(!timer.is_null());
    let timer_ref = unsafe { &mut *timer };
    assert!(timer_ref.type_name() == ObjectClassType::ObjectClassTimer as u8);
    timer_ref.timer_control(TimerControlAction::from_u8(cmd), arg);
    code::EOK.to_errno()
}
