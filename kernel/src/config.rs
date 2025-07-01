// FIXME: We should use kconfig to generate this file.
use crate::types::ThreadPriority;

pub const MAX_THREAD_PRIORITY: ThreadPriority = (ThreadPriority::BITS - 1) as ThreadPriority;
pub const TASKLET_PRIORITY: ThreadPriority = MAX_THREAD_PRIORITY - 1;
pub const TASKLET_STACK_SIZE: usize = 512;

// We must ensure the stack is big enough to contain context and
// to perform computing in the schedule loop.
#[cfg(all(debug_assertions, target_pointer_width = "32"))]
pub const SYSTEM_THREAD_STACK_SIZE: usize = 8 << 10;
#[cfg(all(not(debug_assertions), target_pointer_width = "32"))]
pub const SYSTEM_THREAD_STACK_SIZE: usize = 4 << 10;

#[cfg(all(debug_assertions, target_pointer_width = "32"))]
pub const DEFAULT_STACK_SIZE: usize = 8 << 10;
#[cfg(all(not(debug_assertions), target_pointer_width = "32"))]
pub const DEFAULT_STACK_SIZE: usize = 4 << 10;

#[cfg(all(debug_assertions, target_pointer_width = "64"))]
pub const SYSTEM_THREAD_STACK_SIZE: usize = 32 << 10;
#[cfg(all(not(debug_assertions), target_pointer_width = "64"))]
pub const SYSTEM_THREAD_STACK_SIZE: usize = 4096;

#[cfg(all(debug_assertions, target_pointer_width = "64"))]
pub const DEFAULT_STACK_SIZE: usize = 16 << 10;
#[cfg(all(not(debug_assertions), target_pointer_width = "64"))]
pub const DEFAULT_STACK_SIZE: usize = 8 << 10;

pub const SOFT_TIMER_THREAD_PRIORITY: ThreadPriority = 0;
