use crate::arm_cortex_m::Arch;
use cortex_m::asm;

impl Arch {
    pub fn signal_event() {
        asm::sev();
    }

    pub fn wait_for_event() {
        asm::wfe();
    }

    pub fn wait_for_interrupt() {
        asm::wfi();
    }
}
