use crate::blue_kernel::{error::code, idle};

#[cfg(feature = "idle_hook")]
#[no_mangle]
pub unsafe extern "C" fn rt_thread_idle_sethook(hook: Option<idle::IdleHookFn>) -> i32 {
    if let Some(hook_fn) = hook {
        let res = idle::IDLE_HOOK_LIST.sethook(hook_fn as *mut idle::IdleHookFn);
        if res {
            return code::EOK.to_errno();
        }
    }
    code::EFULL.to_errno()
}

#[cfg(feature = "idle_hook")]
#[no_mangle]
pub unsafe extern "C" fn rt_thread_idle_delhook(hook: Option<idle::IdleHookFn>) -> i32 {
    if let Some(hook_fn) = hook {
        let res = idle::IDLE_HOOK_LIST.delhook(hook_fn as *mut idle::IdleHookFn);
        if res {
            return code::EOK.to_errno();
        }
    }
    code::EFULL.to_errno()
}
