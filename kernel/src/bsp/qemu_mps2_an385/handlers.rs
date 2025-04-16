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

use crate::bsp::uart::{UART0RX_Handler, UART0TX_Handler};
default_irq_handler!(UART1RX_Handler);
default_irq_handler!(UART1TX_Handler);
default_irq_handler!(UART2RX_Handler);
default_irq_handler!(UART2TX_Handler);
default_irq_handler!(GPIO0ALL_Handler);
default_irq_handler!(GPIO1ALL_Handler);
default_irq_handler!(TIMER0_Handler);
default_irq_handler!(TIMER1_Handler);
default_irq_handler!(DUALTIMER_Handler);
default_irq_handler!(SPI_0_1_Handler);
default_irq_handler!(UART_0_1_2_OVF_Handler);
default_irq_handler!(ETHERNET_Handler);
default_irq_handler!(I2S_Handler);
default_irq_handler!(TOUCHSCREEN_Handler);
default_irq_handler!(GPIO2_Handler);
default_irq_handler!(GPIO3_Handler);
default_irq_handler!(UART3RX_Handler);
default_irq_handler!(UART3TX_Handler);
default_irq_handler!(UART4RX_Handler);
default_irq_handler!(UART4TX_Handler);
default_irq_handler!(SPI_2_Handler);
default_irq_handler!(SPI_3_4_Handler);
default_irq_handler!(GPIO0_0_Handler);
default_irq_handler!(GPIO0_1_Handler);
default_irq_handler!(GPIO0_2_Handler);
default_irq_handler!(GPIO0_3_Handler);
default_irq_handler!(GPIO0_4_Handler);
default_irq_handler!(GPIO0_5_Handler);
default_irq_handler!(GPIO0_6_Handler);
default_irq_handler!(GPIO0_7_Handler);

#[doc(hidden)]
#[link_section = ".vector_table.interrupts"]
#[no_mangle]
static __INTERRUPTS: InterruptTable = {
    let mut arr = [Vector { reserved: 0 }; 240];
    arr[0] = Vector {
        handler: UART0RX_Handler,
    }; /*   0 UART 0 receive interrupt */
    arr[1] = Vector {
        handler: UART0TX_Handler,
    }; /*   1 UART 0 transmit interrupt */
    arr[2] = Vector {
        handler: UART1RX_Handler,
    }; /*   2 UART 1 receive interrupt */
    arr[3] = Vector {
        handler: UART1TX_Handler,
    }; /*   3 UART 1 transmit interrupt */
    arr[4] = Vector {
        handler: UART2RX_Handler,
    }; /*   4 UART 2 receive interrupt */
    arr[5] = Vector {
        handler: UART2TX_Handler,
    }; /*   5 UART 2 transmit interrupt */
    arr[6] = Vector {
        handler: GPIO0ALL_Handler,
    }; /*   6 GPIO 0 combined interrupt */
    arr[7] = Vector {
        handler: GPIO1ALL_Handler,
    }; /*   7 GPIO 1 combined interrupt */
    arr[8] = Vector {
        handler: TIMER0_Handler,
    }; /*   8 Timer 0 interrupt */
    arr[9] = Vector {
        handler: TIMER1_Handler,
    }; /*   9 Timer 1 interrupt */
    arr[10] = Vector {
        handler: DUALTIMER_Handler,
    }; /*  10 Dual Timer interrupt */
    arr[11] = Vector {
        handler: SPI_0_1_Handler,
    }; /*  11 SPI 0, SPI 1 interrupt */
    arr[12] = Vector {
        handler: UART_0_1_2_OVF_Handler,
    }; /*  12 UART overflow (0, 1 & 2) interrupt */
    arr[13] = Vector {
        handler: ETHERNET_Handler,
    }; /*  13 Ethernet interrupt */
    arr[14] = Vector {
        handler: I2S_Handler,
    }; /*  14 Audio I2S interrupt */
    arr[15] = Vector {
        handler: TOUCHSCREEN_Handler,
    }; /*  15 Touch Screen interrupt */
    arr[16] = Vector {
        handler: GPIO2_Handler,
    }; /*  16 GPIO 2 combined interrupt */
    arr[17] = Vector {
        handler: GPIO3_Handler,
    }; /*  17 GPIO 3 combined interrupt */
    arr[18] = Vector {
        handler: UART3RX_Handler,
    }; /*  18 UART 3 receive interrupt */
    arr[19] = Vector {
        handler: UART3TX_Handler,
    }; /*  19 UART 3 transmit interrupt */
    arr[20] = Vector {
        handler: UART4RX_Handler,
    }; /*  20 UART 4 receive interrupt */
    arr[21] = Vector {
        handler: UART4TX_Handler,
    }; /*  21 UART 4 transmit interrupt */
    arr[22] = Vector {
        handler: SPI_2_Handler,
    }; /*  22 SPI 2 interrupt */
    arr[23] = Vector {
        handler: SPI_3_4_Handler,
    }; /*  23 SPI 3, SPI 4 interrupt */
    arr[24] = Vector {
        handler: GPIO0_0_Handler,
    }; /*  24 GPIO 0 individual interrupt ( 0) */
    arr[25] = Vector {
        handler: GPIO0_1_Handler,
    }; /*  25 GPIO 0 individual interrupt ( 1) */
    arr[26] = Vector {
        handler: GPIO0_2_Handler,
    }; /*  26 GPIO 0 individual interrupt ( 2) */
    arr[27] = Vector {
        handler: GPIO0_3_Handler,
    }; /*  27 GPIO 0 individual interrupt ( 3) */
    arr[28] = Vector {
        handler: GPIO0_4_Handler,
    }; /*  28 GPIO 0 individual interrupt ( 4) */
    arr[29] = Vector {
        handler: GPIO0_5_Handler,
    }; /*  29 GPIO 0 individual interrupt ( 5) */
    arr[30] = Vector {
        handler: GPIO0_6_Handler,
    }; /*  30 GPIO 0 individual interrupt ( 6) */
    arr[31] = Vector {
        handler: GPIO0_7_Handler,
    }; /*  31 GPIO 0 individual interrupt ( 7) */
    arr
};
