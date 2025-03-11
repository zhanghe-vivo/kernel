use super::Context;
use core::ffi::c_long;

#[inline]
pub extern "C" fn handle(val: c_long) -> c_long {
    val
}

#[inline]
pub unsafe extern "C" fn handle_context(ctx: &Context) -> usize {
    map_args!(ctx.args, 0, val, c_long);
    handle(val) as usize
}
