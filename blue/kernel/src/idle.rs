use core::{
    ffi, mem, ptr,
    sync::atomic::{AtomicPtr, Ordering},
};

use crate::{
    cpu,
    static_init::UnsafeStaticInit,
    str::CStr,
    thread::{ThreadEntryFn, ThreadWithStack},
    zombie,
};
use pinned_init::{pin_data, pin_init, pin_init_array_from_fn, PinInit};
use rt_bindings;

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
                unsafe {
                    let idle_hook: IdleHookFn = mem::transmute(idle_hook);
                    idle_hook();
                }
            }
        }
    }
}

const IDLE_STACK_SIZE: usize = rt_bindings::IDLE_THREAD_STACK_SIZE as usize;
#[pin_data]
pub struct IdleTheads {
    #[pin]
    threads: [ThreadWithStack<IDLE_STACK_SIZE>; cpu::CPUS_NUMBER],
}

static mut IDLE_THREADS: UnsafeStaticInit<IdleTheads, IdleTheadsInit> =
    UnsafeStaticInit::new(IdleTheadsInit);

struct IdleTheadsInit;
unsafe impl PinInit<IdleTheads> for IdleTheadsInit {
    unsafe fn __pinned_init(self, slot: *mut IdleTheads) -> Result<(), core::convert::Infallible> {
        let init = IdleTheads::new();
        unsafe { init.__pinned_init(slot) }
    }
}

impl IdleTheads {
    pub(crate) fn init_once() {
        unsafe {
            (&raw const IDLE_THREADS as *const UnsafeStaticInit<IdleTheads, IdleTheadsInit>)
                .as_ref()
                .unwrap_unchecked()
                .init_once();

            (&raw const zombie::ZOMBIE_MANAGER
                as *const UnsafeStaticInit<zombie::ZombieManager, _>)
                .as_ref()
                .unwrap_unchecked()
                .init_once();

            #[cfg(feature = "RT_USING_SMP")]
            (&raw const zombie::ZOMBIE_MANAGER
                as *const UnsafeStaticInit<zombie::ZombieManager, _>)
                .cast_mut()
                .as_mut()
                .unwrap_unchecked()
                .start_up();

            (&raw const IDLE_THREADS as *const UnsafeStaticInit<IdleTheads, IdleTheadsInit>)
                .cast_mut()
                .as_mut()
                .unwrap_unchecked()
                .start_up();
        }
    }

    #[cfg(feature = "RT_USING_SMP")]
    #[inline]
    pub(crate) fn new() -> impl PinInit<Self> {
        pin_init!(Self {
            threads <- pin_init_array_from_fn(|i| ThreadWithStack::new_with_bind(IDLE_NAME, Self::idle_thread_entry as ThreadEntryFn,
                ptr::null_mut(), (rt_bindings::RT_THREAD_PRIORITY_MAX - 1) as u8, 32, i as u8)),
        })
    }

    // FIXME
    #[cfg(not(feature = "RT_USING_SMP"))]
    #[inline]
    pub(crate) fn new() -> impl PinInit<Self> {
        pin_init!(Self {
            threads <- pin_init_array_from_fn(|_i| ThreadWithStack::new(IDLE_NAME, Self::idle_thread_entry as ThreadEntryFn,
                 ptr::null_mut(), (rt_bindings::RT_THREAD_PRIORITY_MAX - 1) as u8, 32)),
        })
    }

    #[inline]
    pub(crate) fn start_up(&mut self) {
        for i in 0..cpu::CPUS_NUMBER {
            self.threads[i].start();
        }
    }

    extern "C" fn idle_thread_entry(_parameter: *mut ffi::c_void) {
        #[cfg(feature = "RT_USING_SMP")]
        unsafe {
            if Arch::smp::core_id() != 0u8 {
                loop {
                    rt_bindings::rt_hw_secondary_cpu_idle_exec();
                }
            }
        }

        loop {
            #[cfg(not(feature = "RT_USING_SMP"))]
            unsafe {
                (&raw const zombie::ZOMBIE_MANAGER
                    as *const UnsafeStaticInit<zombie::ZombieManager, _>)
                    .as_ref()
                    .unwrap_unchecked()
                    .reclaim()
            };

            #[cfg(feature = "RT_USING_IDLE_HOOK")]
            IDLE_HOOK_LIST.hook_execute();

            // TODO: add power manager
        }
    }
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
