use crate::arch::{asm::DsbOptions, registers::daif::DAIF, Arch};
use spin::Mutex;
use tock_registers::interfaces::{Readable, Writeable};
extern crate alloc;
use alloc::boxed::Box;
use arm_gic::{gicv3::*, IntId};
use bluekernel_kconfig::CPUS_NR;

const GICD: usize = 0x8000000;
const GICR: usize = 0x80a0000;

// The ID of the first Private Peripheral Interrupt.
const PPI_START: u32 = 16;
const SPI_START: u32 = 32;
const SPECIAL_START: u32 = 1020;
const SPECIAL_END: u32 = 1024;

static GIC: Mutex<Option<GicV3>> = Mutex::new(None);

pub use arm_gic::Trigger as IrqTrigger;

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
pub fn init() {
    let mut gic = unsafe {
        GicV3::new(
            GICD as *mut u64 as _,
            GICR as *mut u64 as _,
            CPUS_NR as usize,
            false,
        )
    };
    // Initialize first CPU
    gic.setup(0);
    //set the priority mask for the current CPU core.
    GicV3::set_priority_mask(0x80);
    // Store GIC instance
    *GIC.lock() = Some(gic);
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

pub static IRQ_MANAGER: Mutex<IrqManager> = Mutex::new(IrqManager::new());

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

impl Arch {
    // Disable interrupts
    pub fn disable_interrupts() -> usize {
        let state = DAIF.get();
        unsafe {
            core::arch::asm!(
                "msr daifset, {arg}",
                arg = const 0b0011,
                options(nomem, nostack, preserves_flags)
            );
        }
        Arch::dsb(DsbOptions::Sys);
        state as usize
    }

    // Enable interrupts
    pub fn enable_interrupts(state: usize) {
        Arch::dsb(DsbOptions::Sys);
        DAIF.set(state as u64);
    }

    // Check if interrupts are active
    pub fn is_interrupts_active() -> bool {
        !DAIF.is_set(DAIF::I)
    }

    // Register interrupt handler
    pub fn register_handler(
        irq: IrqNumber,
        handler: Box<dyn IrqHandler>,
    ) -> Result<(), &'static str> {
        IRQ_MANAGER.lock().register_handler(irq, handler)
    }

    // Trigger interrupt
    pub fn trigger_irq(irq: IrqNumber) -> Result<(), &'static str> {
        IRQ_MANAGER.lock().trigger_irq(irq)
    }

    // enable interrupt
    pub fn enable_irq(irq: IrqNumber, cpu_id: usize) {
        if let Some(gic) = &mut *GIC.lock() {
            gic.enable_interrupt(irq.0, Some(cpu_id), true);
        }
    }

    // disable interrupt
    pub fn disable_irq(irq: IrqNumber, cpu_id: usize) {
        if let Some(gic) = &mut *GIC.lock() {
            gic.enable_interrupt(irq.0, Some(cpu_id), false);
        }
    }

    // Set interrupt priority
    pub fn set_interrupt_priority(irq: IrqNumber, cpu_id: usize, priority: u8) {
        if let Some(gic) = &mut *GIC.lock() {
            gic.set_interrupt_priority(irq.0, Some(cpu_id), priority);
        }
    }

    // Set priority mask for current CPU
    pub fn set_priority_mask(priority: u8) {
        GicV3::set_priority_mask(priority);
    }

    // Configures the trigger type for the interrupt with the given ID
    pub fn set_trigger(irq: IrqNumber, cpu_id: usize, trigger: IrqTrigger) {
        if let Some(gic) = &mut *GIC.lock() {
            gic.set_trigger(irq.0, Some(cpu_id), trigger);
        }
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
}
