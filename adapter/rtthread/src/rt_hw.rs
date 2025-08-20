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

use crate::rt_def::*;
use blueos::{arch, irq, scheduler};

// rt_base_t rt_hw_interrupt_disable(void);
#[no_mangle]
pub extern "C" fn rt_hw_interrupt_disable() -> rt_base_t {
    arch::disable_local_irq_save() as rt_base_t
}

// void rt_hw_interrupt_enable(rt_base_t level);
#[no_mangle]
pub extern "C" fn rt_hw_interrupt_enable(level: rt_base_t) {
    arch::enable_local_irq_restore(level as usize);
}

// rt_uint8_t rt_interrupt_get_nest(void);
#[no_mangle]
pub extern "C" fn rt_interrupt_get_nest() -> rt_uint8_t {
    irq::irq_nesting_count() as rt_uint8_t
}

// void rt_enter_critical(void);
#[no_mangle]
pub extern "C" fn rt_enter_critical() {
    arch::disable_local_irq();
    let current = scheduler::current_thread();
    let _ = current.disable_preempt();
}

// void rt_exit_critical(void);
#[no_mangle]
pub extern "C" fn rt_exit_critical() {
    let current = scheduler::current_thread();
    let _ = current.enable_preempt();
    arch::enable_local_irq();
}
