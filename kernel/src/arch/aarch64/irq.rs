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

extern crate alloc;

use crate::sync::SpinLock;
use alloc::boxed::Box;
use arm_gic::{gicv3::*, IntId};
use spin::Once;
use tock_registers::interfaces::Readable;

pub use arm_gic::Trigger as IrqTrigger;

// aarch64 irq priority is 0-255
#[derive(Debug, Copy, Clone)]
#[repr(u8)]
pub enum Priority {
    // can't use ipc in high priority irq
    High = 0x10,
    Normal = 0x80,
    Low = 0xf0,
}

// The ID of the first Private Peripheral Interrupt.
const PPI_START: u32 = 16;
const SPI_START: u32 = 32;
const SPECIAL_START: u32 = 1020;
const SPECIAL_END: u32 = 1024;

static GIC: Once<SpinLock<GicV3>> = Once::new();

#[derive(Debug, Copy, Clone, Eq, Ord, PartialOrd, PartialEq)]
#[repr(transparent)]
pub struct IrqNumber(IntId);

impl IrqNumber {
    pub const fn new(irq: u32) -> Self {
        let id = match irq {
            0..PPI_START => IntId::sgi(irq),
            PPI_START..SPI_START => IntId::ppi(irq - PPI_START),
            _ => IntId::spi(irq - SPI_START),
        };
        Self(id)
    }
}
// IrqNumber to u32
impl From<IrqNumber> for u32 {
    fn from(irq: IrqNumber) -> Self {
        u32::from(irq.0)
    }
}

// IrqNumber to usize
impl From<IrqNumber> for usize {
    fn from(irq: IrqNumber) -> Self {
        u32::from(irq.0) as usize
    }
}

// Initialize the GIC for the system
pub unsafe fn init(gicd: u64, gicr: u64, num_cores: usize, is_v4: bool) {
    GIC.call_once(|| {
        // Safety: gicd and gicr must need to be valid pointers.
        let mut gic = unsafe {
            GicV3::new(
                gicd as *mut u64 as _,
                gicr as *mut u64 as _,
                num_cores,
                is_v4,
            )
        };
        // Initialize first CPU
        gic.setup(0);
        //set the priority mask for the current CPU core.
        set_priority_mask(0xff);
        SpinLock::new(gic)
    });
}

fn get_gic() -> &'static SpinLock<GicV3<'static>> {
    GIC.get().unwrap()
}

pub const INTERRUPT_TABLE_LEN: usize = 128;

pub struct IrqContext {
    pub irq: IrqNumber,
    pub handler: Box<dyn IrqHandler>,
}

impl core::fmt::Display for IrqContext {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "IRQ {:?} -> Handler@{:p}", self.irq, self.handler)
    }
}

pub trait IrqHandler: Send + Sync {
    fn handle(&mut self);
}

impl IrqContext {
    fn new(irq: IrqNumber, handler: Box<dyn IrqHandler>) -> Self {
        Self { irq, handler }
    }
}

pub struct IrqManager {
    pub contexts: [Option<IrqContext>; INTERRUPT_TABLE_LEN],
}

pub static IRQ_MANAGER: SpinLock<IrqManager> = SpinLock::new(IrqManager::new());

impl IrqManager {
    const fn new() -> Self {
        const NONE_CONTEXT: Option<IrqContext> = None;
        Self {
            contexts: [NONE_CONTEXT; INTERRUPT_TABLE_LEN],
        }
    }

    fn register_handler(
        &mut self,
        irq: IrqNumber,
        handler: Box<dyn IrqHandler>,
    ) -> Result<(), &'static str> {
        if u32::from(irq) >= INTERRUPT_TABLE_LEN as u32 {
            return Err("IRQ number out of range");
        }
        self.contexts[usize::from(irq)] = Some(IrqContext::new(irq, handler));
        Ok(())
    }

    fn trigger_irq(&mut self, irq: IrqNumber) -> Result<(), &'static str> {
        if let Some(context) = &mut self.contexts[usize::from(irq)] {
            return Ok(context.handler.handle());
        }
        Err("handler not found")
    }
}

// Register interrupt handler
pub fn register_handler(irq: IrqNumber, handler: Box<dyn IrqHandler>) -> Result<(), &'static str> {
    IRQ_MANAGER.lock().register_handler(irq, handler)
}

// Trigger interrupt
pub fn trigger_irq(irq: IrqNumber) -> Result<(), &'static str> {
    IRQ_MANAGER.lock().trigger_irq(irq)
}

// enable interrupt
pub fn enable_irq_with_priority(irq: IrqNumber, cpu_id: usize, priority: Priority) {
    let mut gic = get_gic().irqsave_lock();
    gic.set_interrupt_priority(irq.0, Some(cpu_id), priority as u8);
    gic.enable_interrupt(irq.0, Some(cpu_id), true);
}

pub fn enable_irq(irq: IrqNumber, cpu_id: usize) {
    get_gic()
        .irqsave_lock()
        .enable_interrupt(irq.0, Some(cpu_id), true);
}

// disable interrupt
pub fn disable_irq(irq: IrqNumber, cpu_id: usize) {
    get_gic()
        .irqsave_lock()
        .enable_interrupt(irq.0, Some(cpu_id), false);
}

// Set interrupt priority
pub fn set_irq_priority(irq: IrqNumber, cpu_id: usize, priority: u8) {
    get_gic()
        .irqsave_lock()
        .set_interrupt_priority(irq.0, Some(cpu_id), priority);
}

// Set priority mask for current CPU
pub fn set_priority_mask(priority: u8) {
    GicV3::set_priority_mask(priority);
}

// Configures the trigger type for the interrupt with the given ID
pub fn set_trigger(irq: IrqNumber, cpu_id: usize, trigger: IrqTrigger) {
    get_gic()
        .irqsave_lock()
        .set_trigger(irq.0, Some(cpu_id), trigger);
}

// Get and acknowledge pending interrupt
pub fn get_interrupt() -> IrqNumber {
    match GicV3::get_and_acknowledge_interrupt() {
        None => IrqNumber(IntId::SPECIAL_NONE),
        Some(intid) => IrqNumber(intid),
    }
}

// End interrupt processing
pub fn end_interrupt(irq: IrqNumber) {
    GicV3::end_interrupt(irq.0);
}

pub fn send_sgi(irq: IrqNumber, cpu_mask: u16) {
    GicV3::send_sgi(
        irq.0,
        SgiTarget::List {
            affinity3: 0,
            affinity2: 0,
            affinity1: 0,
            target_list: cpu_mask,
        },
    );
}
