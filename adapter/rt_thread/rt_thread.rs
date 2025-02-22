use crate::blue_kernel::{
    alloc::boxed::Box,
    clock,
    cpu::Cpu,
    error::code,
    object::{KernelObject, ObjectClassType},
    process,
    thread::{SuspendFlag, Thread, ThreadCleanupFn, ThreadEntryFn},
};
use blue_infra::klibc;
use core::{
    ffi,
    pin::Pin,
    ptr::{self, NonNull},
};
use pinned_init::PinInit;

#[no_mangle]
pub extern "C" fn rt_thread_init(
    thread: *mut Thread,
    name: *const ffi::c_char,
    entry: ThreadEntryFn,
    parameter: *mut ffi::c_void,
    stack_start: *mut ffi::c_void,
    stack_size: u32,
    priority: u8,
    tick: u32,
) -> i32 {
    // parameter check
    assert!(!thread.is_null());
    assert!(!stack_start.is_null());
    assert!(tick != 0);

    let name_cstr = unsafe { ffi::CStr::from_ptr(name) };
    let init = Thread::static_new(
        name_cstr,
        entry,
        parameter as *mut usize,
        stack_start as *mut u8,
        stack_size as usize,
        priority,
        tick,
    );
    // no Error
    unsafe {
        let _ = init.__pinned_init(thread);
    }
    return code::EOK.to_errno();
}

#[no_mangle]
pub extern "C" fn rt_thread_self() -> *mut Thread {
    match Cpu::get_current_thread() {
        Some(thread) => thread.as_ptr(),
        None => ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn rt_thread_startup(thread: *mut Thread) -> i32 {
    // parameter check
    assert!(!thread.is_null());
    let th_mut = unsafe { &mut *thread };
    assert!(th_mut.type_name() == ObjectClassType::ObjectClassThread as u8);
    assert!(th_mut.stat.is_init());
    th_mut.start();

    return code::EOK.to_errno();
}

#[no_mangle]
pub extern "C" fn rt_thread_close(thread: *mut Thread) -> i32 {
    // parameter check
    assert!(!thread.is_null());
    let th_mut = unsafe { &mut *thread };
    assert!(th_mut.type_name() == ObjectClassType::ObjectClassThread as u8);
    th_mut.close();

    return code::EOK.to_errno();
}

#[no_mangle]
pub extern "C" fn rt_thread_detach(thread: *mut Thread) -> i32 {
    // parameter check
    assert!(!thread.is_null());
    let th = unsafe { &mut *thread };
    assert!(th.type_name() == ObjectClassType::ObjectClassThread as u8);
    th.detach();
    return code::EOK.to_errno();
}

// #[cfg(feature = "heap")]
#[no_mangle]
pub extern "C" fn rt_thread_create(
    name: *const ffi::c_char,
    entry: ThreadEntryFn,
    parameter: *mut ffi::c_void,
    stack_size: u32,
    priority: u8,
    tick: u32,
) -> *mut Thread {
    let name_cstr = unsafe { ffi::CStr::from_ptr(name) };

    let thread = Thread::try_new_in_heap(
        name_cstr,
        entry,
        parameter as *mut usize,
        stack_size as usize,
        priority,
        tick,
    );
    match thread {
        Ok(th) => {
            // need to free by zombie.
            unsafe { Box::leak(Pin::into_inner_unchecked(th)) }
        }
        Err(_) => return ptr::null_mut(),
    }
}

#[cfg(feature = "heap")]
#[no_mangle]
pub extern "C" fn rt_thread_delete(thread: *mut Thread) -> i32 {
    assert!(!thread.is_null());
    let th = unsafe { &mut *thread };
    assert!(th.type_name() == ObjectClassType::ObjectClassThread as u8);
    assert!(!th.is_static_kobject());
    th.detach();
    return code::EOK.to_errno();
}

#[no_mangle]
pub extern "C" fn rt_thread_yield() -> i32 {
    Thread::yield_now();
    return code::EOK.to_errno();
}

#[no_mangle]
pub extern "C" fn rt_thread_delay(tick: u32) -> i32 {
    match Thread::sleep(tick) {
        Ok(_) => return code::EOK.to_errno(),
        Err(e) => e.to_errno(),
    }
}

#[no_mangle]
pub extern "C" fn rt_thread_mdelay(ms: i32) -> i32 {
    let tick = clock::tick_from_millisecond(ms);
    match Thread::sleep(tick) {
        Ok(_) => return code::EOK.to_errno(),
        Err(e) => e.to_errno(),
    }
}

#[derive(PartialEq)]
pub enum ThreadControlAction {
    ThreadCtrlStartup = 0,
    ThreadCtrlClose,
    ThreadCtrlChagePriority,
    ThreadCtrlInfo,
    ThreadCtrlBindCpu,
}

#[no_mangle]
pub extern "C" fn rt_thread_control(thread: *mut Thread, cmd: u32, arg: *mut ffi::c_void) -> i32 {
    assert!(!thread.is_null());
    let th = unsafe { &mut *thread };
    assert!(th.type_name() == ObjectClassType::ObjectClassThread as u8);
    match cmd {
        val if val == ThreadControlAction::ThreadCtrlChagePriority as u32 => {
            let priority_ptr = NonNull::new(arg as *mut u8);
            if let Some(ptr) = priority_ptr {
                let priority = unsafe { *ptr.as_ref() };
                th.change_priority(priority);
            } else {
                return code::EINVAL.to_errno();
            }
        }
        val if val == ThreadControlAction::ThreadCtrlStartup as u32 => {
            th.start();
        }
        val if val == ThreadControlAction::ThreadCtrlClose as u32 => {
            // detach will trigger schedule
            th.detach();
        }
        #[cfg(feature = "smp")]
        val if val == ThreadControlAction::ThreadCtrlBindCpu as u32 => {
            let cpu_ptr = NonNull::new(arg as *mut u8);
            if let Some(ptr) = cpu_ptr {
                let cpu = unsafe { *ptr.as_ref() };
                th.bind_to_cpu(cpu);
            } else {
                return code::EINVAL.to_errno();
            }
        }

        _ => {
            return code::EINVAL.to_errno();
        }
    }

    code::EOK.to_errno()
}

#[no_mangle]
pub extern "C" fn rt_thread_find(name: *mut ffi::c_char) -> *mut Thread {
    return process::find_object(ObjectClassType::ObjectClassThread as u8, name) as *mut Thread;
}

#[no_mangle]
pub extern "C" fn rt_thread_get_name(
    thread: *mut Thread,
    name: *mut ffi::c_char,
    name_size: u8,
) -> i32 {
    return if thread.is_null() {
        code::EINVAL.to_errno()
    } else {
        let th = unsafe { &mut *thread };
        unsafe { klibc::strncpy(name as *mut _, th.parent.name.as_ptr(), name_size as usize) };
        code::EOK.to_errno()
    };
}

#[no_mangle]
pub extern "C" fn rt_thread_suspend_with_flag(thread: *mut Thread, suspend_flag: u32) -> i32 {
    assert!(!thread.is_null());
    let th = unsafe { &mut *thread };
    assert!(th.type_name() == ObjectClassType::ObjectClassThread as u8);
    if th.suspend(SuspendFlag::from_u8(suspend_flag as u8)) {
        return code::EOK.to_errno();
    }
    code::ERROR.to_errno()
}

#[no_mangle]
pub extern "C" fn rt_thread_suspend(thread: *mut Thread) -> i32 {
    rt_thread_suspend_with_flag(thread, SuspendFlag::Uninterruptible as u32)
}

#[no_mangle]
pub extern "C" fn rt_thread_resume(thread: *mut Thread) -> i32 {
    assert!(!thread.is_null());
    let th = unsafe { &mut *thread };
    assert!(th.type_name() == ObjectClassType::ObjectClassThread as u8);
    if th.resume() {
        code::EOK.to_errno()
    } else {
        code::ERROR.to_errno()
    }
}

#[no_mangle]
pub extern "C" fn rt_thread_cleanup(thread: *mut Thread, cleanup: ThreadCleanupFn) {
    assert!(!thread.is_null());
    assert!(cleanup as *const () != core::ptr::null());

    unsafe { (*thread).cleanup = cleanup };
}
