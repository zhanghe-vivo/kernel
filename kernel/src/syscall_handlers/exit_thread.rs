use super::Context;
use core::ffi::c_long;

#[inline]
pub fn handle() -> c_long {
    unsafe { crate::thread::Thread::exit() };
    // If control reaches here, definitely error occurs, return -1 to indicate that.
    return -1;
}

pub fn handle_context(ctx: &Context) -> c_long {
    let t = unsafe { crate::current_thread!().unwrap().as_mut() };
    t.detach();
    return 0;
}
