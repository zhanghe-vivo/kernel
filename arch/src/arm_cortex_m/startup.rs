use core::ptr;
use cortex_m::{asm, peripheral::SCB};

#[repr(C)]
struct CopyTable {
    src: *const u32,
    dest: *mut u32,
    wlen: u32,
}

#[repr(C)]
struct ZeroTable {
    dest: *mut u32,
    wlen: u32,
}

#[no_mangle]
pub unsafe fn reset_handler_inner() -> ! {
    extern "C" {
        static __vector_table: u32;

        #[cfg(all(armv8m, feature = "cmse"))]
        static __StackSeal: u32;

        static __copy_table_start__: CopyTable;
        static __copy_table_end__: CopyTable;
        static __zero_table_start__: ZeroTable;
        static __zero_table_end__: ZeroTable;

        fn _startup() -> !;
    }

    let scb = &*SCB::PTR;
    scb.vtor.write(&__vector_table as *const _ as u32);

    #[cfg(all(armv8m, feature = "cmse"))]
    {
        const TZ_STACK_SEAL_VALUE: u64 = 0xFEF5EDA5FEF5EDA5;
        *(&__StackSeal as *const _ as *mut u64) = TZ_STACK_SEAL_VALUE;
    }

    #[cfg(feature = "unaligned_support_disable")]
    {
        const SCB_CCR_UNALIGN_TRP_MASK: u32 = 1 << 3;
        scb.ccr.write(scb.ccr.read() | SCB_CCR_UNALIGN_TRP_MASK);
    }

    // copy and zero datas
    // let mut p_table = &__copy_table_start__ as *const CopyTable;
    // while p_table < &__copy_table_end__ as *const CopyTable {
    //     let table = &(*p_table);
    //     for i in 0..table.wlen {
    //         ptr::write(
    //             table.dest.add(i as usize),
    //             ptr::read(table.src.add(i as usize)),
    //         );
    //     }
    //     p_table = p_table.add(1);
    // }

    let mut p_table = &__zero_table_start__ as *const ZeroTable;
    while p_table < &__zero_table_end__ as *const ZeroTable {
        let table = &*p_table;
        for i in 0..table.wlen {
            ptr::write(table.dest.add(i as usize), 0);
        }
        p_table = p_table.add(1);
    }

    // call the kernel's entry point
    _startup()
}
