use core::{
    ffi, ptr,
    sync::atomic::{AtomicPtr, Ordering},
};

use crate::{
    cpu, rt_bindings,
    str::CStr,
    thread::{ThreadEntryFn, ThreadWithStack},
    zombie,
};
use pinned_init::*;

const IDLE_NAME: &'static CStr = crate::c_str!("Idle");

#[cfg(feature = "RT_USING_IDLE_HOOK")]
type IdleHookFn = unsafe extern "C" fn();

#[cfg(feature = "RT_USING_IDLE_HOOK")]
static IDLE_HOOK_LIST: IdleHooks = IdleHooks::new();
#[cfg(feature = "RT_USING_IDLE_HOOK")]
const IDLE_HOOK_LIST_SIZE: usize = rt_bindings::RT_IDLE_HOOK_LIST_SIZE as usize;

#[cfg(feature = "RT_USING_IDLE_HOOK")]
struct IdleHooks {
    hooks: [AtomicPtr<IdleHookFn>; IDLE_HOOK_LIST_SIZE],
}

#[cfg(feature = "RT_USING_IDLE_HOOK")]
impl IdleHooks {
    const ARRAY_REPEAT_VALUE: AtomicPtr<IdleHookFn> = AtomicPtr::new(ptr::null_mut());
    pub const fn new() -> Self {
        IdleHooks {
            hooks: [Self::ARRAY_REPEAT_VALUE; IDLE_HOOK_LIST_SIZE],
        }
    }

    pub(crate) fn sethook(&self, hook: *mut IdleHookFn) -> bool {
        for i in 0..IDLE_HOOK_LIST_SIZE {
            let idle_hook = self.hooks[i].load(Ordering::Relaxed);
            if idle_hook.is_null() {
                self.hooks[i].store(hook, Ordering::Release);
                return true;
            }
        }
        false
    }

    pub(crate) fn delhook(&self, hook: *mut IdleHookFn) -> bool {
        for i in 0..IDLE_HOOK_LIST_SIZE {
            let idle_hook = self.hooks[i].load(Ordering::Relaxed);
            if idle_hook == hook {
                self.hooks[i].store(ptr::null_mut() as *mut IdleHookFn, Ordering::Release);
                return true;
            }
        }
        false
    }

    pub(crate) fn hook_execute(&self) {
        for i in 0..IDLE_HOOK_LIST_SIZE {
            let idle_hook = self.hooks[i].load(Ordering::Relaxed);
            if !idle_hook.is_null() {
                unsafe { (*idle_hook)() };
            }
        }
    }
}

#[pin_data]
pub struct IdleThread<const STACK_SIZE: usize> {
    #[pin]
    thread: ThreadWithStack<STACK_SIZE>,
}

impl<const STACK_SIZE: usize> IdleThread<STACK_SIZE> {
    #[inline]
    pub(crate) fn new(cpu: u8) -> impl PinInit<Self> {
        pin_init!(Self {
            thread <- ThreadWithStack::new_with_bind(IDLE_NAME, Self::idle_thread_entry as ThreadEntryFn,
                 ptr::null_mut(), (rt_bindings::RT_THREAD_PRIORITY_MAX - 1) as u8, 32, cpu),
        })
    }

    #[inline]
    pub(crate) fn start(&mut self) {
        self.thread.start();
    }

    extern "C" fn idle_thread_entry(_parameter: *mut ffi::c_void) {
        #[cfg(RT_USING_SMP)]
        if rt_bindings::rt_hw_cpu_id() != 0 {
            loop {
                rt_bindings::rt_hw_secondary_cpu_idle_exec();
            }
        }

        loop {
            #[cfg(feature = "RT_USING_IDLE_HOOK")]
            IDLE_HOOK_LIST.hook_execute();

            #[cfg(not(feature = "RT_USING_SMP"))]
            unsafe {
                zombie::ZOMBIE_MANAGER.reclaim()
            };

            #[cfg(feature = "RT_USING_PM")]
            unsafe {
                rt_bindings::rt_system_power_manager()
            };
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rt_thread_idle_init() {
    zombie::ZOMBIE_MANAGER.init_once();
    #[cfg(feature = "RT_USING_SMP")]
    zombie::ZOMBIE_MANAGER.start_up();
    // idle is inited in cpu init.
    cpu::Cpus::start_idle_threads();
}

#[cfg(feature = "RT_USING_IDLE_HOOK")]
#[no_mangle]
pub unsafe extern "C" fn rt_thread_idle_sethook(hook: Option<IdleHookFn>) -> rt_bindings::rt_err_t {
    if let Some(hook_fn) = hook {
        let res = IDLE_HOOK_LIST.sethook(hook_fn as *mut IdleHookFn);
        if res {
            return rt_bindings::RT_EOK as rt_bindings::rt_err_t;
        }
    }
    -(rt_bindings::RT_EFULL as rt_bindings::rt_err_t)
}

#[cfg(feature = "RT_USING_IDLE_HOOK")]
#[no_mangle]
pub unsafe extern "C" fn rt_thread_idle_delhook(hook: Option<IdleHookFn>) -> rt_bindings::rt_err_t {
    if let Some(hook_fn) = hook {
        let res = IDLE_HOOK_LIST.delhook(hook_fn as *mut IdleHookFn);
        if res {
            return rt_bindings::RT_EOK as rt_bindings::rt_err_t;
        }
    }
    -(rt_bindings::RT_EFULL as rt_bindings::rt_err_t)
}
