use crate::arch::{InterruptTable, Vector};

macro_rules! default_irq_handler {
    ($handler_name:ident) => {
        #[link_section = ".text.vector_handlers"]
        #[linkage = "weak"]
        #[no_mangle]
        pub unsafe extern "C" fn $handler_name() {
            crate::println!("{}", stringify!($handler_name));
        }
    };
}

use crate::bsp::{UARTRX0_Handler, UARTTX0_Handler};
default_irq_handler!(NONSEC_WATCHDOG_RESET_REQ_Handler);
default_irq_handler!(NONSEC_WATCHDOG_Handler);
default_irq_handler!(SLOWCLK_Timer_Handler);
default_irq_handler!(TFM_TIMER0_IRQ_Handler);
default_irq_handler!(TIMER1_Handler);
default_irq_handler!(TIMER2_Handler);
default_irq_handler!(MPC_Handler);
default_irq_handler!(PPC_Handler);
default_irq_handler!(MSC_Handler);
default_irq_handler!(BRIDGE_ERROR_Handler);
default_irq_handler!(MGMT_PPU_Handler);
default_irq_handler!(SYS_PPU_Handler);
default_irq_handler!(CPU0_PPU_Handler);
default_irq_handler!(DEBUG_PPU_Handler);
default_irq_handler!(TIMER3_AON_Handler);
default_irq_handler!(CPU0_CTI_0_Handler);
default_irq_handler!(CPU0_CTI_1_Handler);
default_irq_handler!(System_Timestamp_Counter_Handler);
default_irq_handler!(UARTRX1_Handler);
default_irq_handler!(UARTTX1_Handler);
default_irq_handler!(UARTRX2_Handler);
default_irq_handler!(UARTTX2_Handler);
default_irq_handler!(UARTRX3_Handler);
default_irq_handler!(UARTTX3_Handler);
default_irq_handler!(UARTRX4_Handler);
default_irq_handler!(UARTTX4_Handler);

#[doc(hidden)]
#[link_section = ".vector_table.interrupts"]
#[no_mangle]
static __INTERRUPTS: InterruptTable = {
    let mut arr = [Vector { reserved: 0 }; 496];
    arr[0] = Vector {
        handler: NONSEC_WATCHDOG_RESET_REQ_Handler,
    };
    arr[1] = Vector {
        handler: NONSEC_WATCHDOG_Handler,
    };
    arr[2] = Vector {
        handler: SLOWCLK_Timer_Handler,
    };
    arr[3] = Vector {
        handler: TFM_TIMER0_IRQ_Handler,
    };
    arr[4] = Vector {
        handler: TIMER1_Handler,
    };
    arr[5] = Vector {
        handler: TIMER2_Handler,
    };
    arr[6] = Vector { reserved: 0 };
    arr[7] = Vector { reserved: 0 };
    arr[8] = Vector { reserved: 0 };
    arr[9] = Vector {
        handler: MPC_Handler,
    };
    arr[10] = Vector {
        handler: PPC_Handler,
    };
    arr[11] = Vector {
        handler: MSC_Handler,
    };
    arr[12] = Vector {
        handler: BRIDGE_ERROR_Handler,
    };
    arr[13] = Vector { reserved: 0 };
    arr[14] = Vector {
        handler: MGMT_PPU_Handler,
    };
    arr[15] = Vector {
        handler: SYS_PPU_Handler,
    };
    arr[16] = Vector {
        handler: CPU0_PPU_Handler,
    };
    arr[17] = Vector { reserved: 0 };
    arr[18] = Vector { reserved: 0 };
    arr[19] = Vector { reserved: 0 };
    arr[20] = Vector { reserved: 0 };
    arr[21] = Vector { reserved: 0 };
    arr[22] = Vector { reserved: 0 };
    arr[23] = Vector { reserved: 0 };
    arr[24] = Vector { reserved: 0 };
    arr[25] = Vector {
        handler: DEBUG_PPU_Handler,
    };
    arr[27] = Vector {
        handler: TIMER3_AON_Handler,
    };
    arr[28] = Vector {
        handler: CPU0_CTI_0_Handler,
    };
    arr[29] = Vector {
        handler: CPU0_CTI_1_Handler,
    };
    arr[30] = Vector { reserved: 0 };
    arr[31] = Vector { reserved: 0 };
    arr[32] = Vector {
        handler: System_Timestamp_Counter_Handler,
    };
    // In the new version of QEMU (9.20), the UART RX interrupt and TX interrupt have been swapped.
    // For details, see `fix RX/TX interrupts order <https://github.com/qemu/qemu/commit/5a558be93ad628e5bed6e0ee062870f49251725c>`_
    arr[33] = Vector {
        handler: UARTTX0_Handler,
    };
    arr[34] = Vector {
        handler: UARTRX0_Handler,
    };
    arr[35] = Vector {
        handler: UARTRX1_Handler,
    };
    arr[36] = Vector {
        handler: UARTTX1_Handler,
    };
    arr[37] = Vector {
        handler: UARTRX2_Handler,
    };
    arr[38] = Vector {
        handler: UARTTX2_Handler,
    };
    arr[39] = Vector {
        handler: UARTRX3_Handler,
    };
    arr[40] = Vector {
        handler: UARTTX3_Handler,
    };
    arr[41] = Vector {
        handler: UARTRX4_Handler,
    };
    arr[42] = Vector {
        handler: UARTTX4_Handler,
    };

    arr
};
