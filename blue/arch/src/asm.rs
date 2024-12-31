#[cfg(cortex_m)]
pub use cortex_m::asm;

pub fn signal_event() {
    asm::sev();
}

pub fn wait_for_event() {
    asm::wfe();
}
