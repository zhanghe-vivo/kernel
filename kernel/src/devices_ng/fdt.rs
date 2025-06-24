use crate::println;
use flat_device_tree::Fdt;
use spin::Once;

static FDT: Once<Fdt<'static>> = Once::new();

pub fn fdt_init(base: u64) {
    println!("FDT address: 0x{:x}", base);
    // SAFETY: We trust that the FDT pointer we were given is valid, and this is the only time we
    // use it.
    let fdt = unsafe { Fdt::from_ptr(base as *const u8).unwrap() };
    println!("FDT size: {} bytes", fdt.total_size());
    println!("FDT: {:?}", fdt);
    for reserved in fdt.memory_reservations() {
        println!("Reserved memory: {:?}", reserved);
    }
    FDT.call_once(|| fdt);
}

pub fn get_fdt() -> &'static Fdt<'static> {
    FDT.get().unwrap()
}
