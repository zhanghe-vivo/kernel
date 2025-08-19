// Copyright (c) 2025 vivo Mobile Communication Co., Ltd.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//       http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::{arch, scheduler, thread};
use libc::SIGTERM;

fn handle_sigterm(signum: i32) {
    scheduler::retire_me();
}

fn handle_signal(signum: i32) {
    if signum != SIGTERM {
        return;
    }
    handle_sigterm(signum);
}

// This routine is supposed to be executed in THREAD mode.
#[inline(never)]
pub(crate) unsafe extern "C" fn handler_entry(_sp: usize, _old_sp: usize) {
    let current = scheduler::current_thread();
    let sigset = current.pending_signals();
    for i in 0..32 {
        if sigset & (1 << i) == 0 {
            continue;
        }
        handle_signal(i as i32);
        current.clear_signal(i);
    }
    {
        let mut l = current.lock();
        l.deactivate_signal_context();
    }
    let saved_sp = current.saved_sp();
    current.transfer_state(thread::RUNNING, thread::READY);
    let mut hook_holder = scheduler::ContextSwitchHookHolder::new(current);
    // We are switching from current thread's signal context to its thread
    // context.
    arch::restore_context_with_hook(saved_sp, &mut hook_holder as *mut _);
}
