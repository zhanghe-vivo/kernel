use core::{
    ffi, mem, ptr,
    sync::atomic::{AtomicPtr, Ordering},
};

use crate::{
    cpu,
    static_init::UnsafeStaticInit,
    thread::{ThreadEntryFn, ThreadWithStack},
    zombie,
};
use core::ffi::CStr;
use pinned_init::{pin_data, pin_init, pin_init_array_from_fn, PinInit};

const IDLE_NAME: &'static CStr = crate::c_str!("Idle");

#[cfg(feature = "idle_hook")]
pub type IdleHookFn = unsafe extern "C" fn();

#[cfg(feature = "idle_hook")]
pub static IDLE_HOOK_LIST: IdleHooks = IdleHooks::new();
#[cfg(feature = "idle_hook")]
const IDLE_HOOK_LIST_SIZE: usize = 4;

#[cfg(feature = "idle_hook")]
pub struct IdleHooks {
    hooks: [AtomicPtr<IdleHookFn>; IDLE_HOOK_LIST_SIZE],
}

#[cfg(feature = "idle_hook")]
impl IdleHooks {
    const ARRAY_REPEAT_VALUE: AtomicPtr<IdleHookFn> = AtomicPtr::new(ptr::null_mut());
    pub const fn new() -> Self {
        IdleHooks {
            hooks: [Self::ARRAY_REPEAT_VALUE; IDLE_HOOK_LIST_SIZE],
        }
    }

    pub fn sethook(&self, hook: *mut IdleHookFn) -> bool {
        for i in 0..IDLE_HOOK_LIST_SIZE {
            let idle_hook = self.hooks[i].load(Ordering::Acquire);
            if idle_hook.is_null() {
                self.hooks[i].store(hook, Ordering::Release);
                return true;
            }
        }
        false
    }

    pub fn delhook(&self, hook: *mut IdleHookFn) -> bool {
        for i in 0..IDLE_HOOK_LIST_SIZE {
            let idle_hook = self.hooks[i].load(Ordering::Acquire);
            if idle_hook == hook {
                self.hooks[i].store(ptr::null_mut() as *mut IdleHookFn, Ordering::Release);
                return true;
            }
        }
        false
    }

    pub(crate) fn hook_execute(&self) {
        for i in 0..IDLE_HOOK_LIST_SIZE {
            let idle_hook = self.hooks[i].load(Ordering::Acquire);
            if !idle_hook.is_null() {
                unsafe {
                    let idle_hook: IdleHookFn = mem::transmute(idle_hook);
                    idle_hook();
                }
            }
        }
    }
}

const IDLE_STACK_SIZE: usize = 2048;
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

            #[cfg(feature = "smp")]
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

    #[cfg(feature = "smp")]
    #[inline]
    pub(crate) fn new() -> impl PinInit<Self> {
        pin_init!(Self {
            threads <- pin_init_array_from_fn(|i| ThreadWithStack::new_with_bind(IDLE_NAME, Self::idle_thread_entry as ThreadEntryFn,
                ptr::null_mut(), (bluekernel_kconfig::THREAD_PRIORITY_MAX - 1) as u8, 32, i as u8)),
        })
    }

    // FIXME
    #[cfg(not(feature = "smp"))]
    #[inline]
    pub(crate) fn new() -> impl PinInit<Self> {
        pin_init!(Self {
            threads <- pin_init_array_from_fn(|_i| ThreadWithStack::new(IDLE_NAME, Self::idle_thread_entry as ThreadEntryFn,
                 ptr::null_mut(), (bluekernel_kconfig::THREAD_PRIORITY_MAX - 1) as u8, 32)),
        })
    }

    #[inline]
    pub(crate) fn start_up(&mut self) {
        for i in 0..cpu::CPUS_NUMBER {
            self.threads[i].start();
        }
    }

    extern "C" fn idle_thread_entry(_parameter: *mut ffi::c_void) {
        #[cfg(feature = "smp")]
        unsafe {
            if Arch::smp::core_id() != 0u8 {
                loop {
                    // TODO: call for libcpu
                    // rt_bindings::rt_hw_secondary_cpu_idle_exec();
                }
            }
        }

        loop {
            #[cfg(not(feature = "smp"))]
            unsafe {
                (&raw const zombie::ZOMBIE_MANAGER
                    as *const UnsafeStaticInit<zombie::ZombieManager, _>)
                    .cast_mut()
                    .as_mut()
                    .unwrap_unchecked()
                    .reclaim()
            };

            #[cfg(feature = "idle_hook")]
            IDLE_HOOK_LIST.hook_execute();

            // TODO: add power manager
        }
    }
}

/// bindgen for idle hook
#[allow(improper_ctypes_definitions)]
#[no_mangle]
pub extern "C" fn bindgen_idle_hook(_hook: IdleHookFn) {
    0;
}
