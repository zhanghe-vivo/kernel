/******************************************************************************
 * @file     startup_ARMCM55.c
 * @brief    CMSIS-Core Device Startup File for Cortex-M55 Device
 * @version  V1.1.0
 * @date     16. December 2020
 ******************************************************************************/
/*
 * Copyright (c) 2020 Arm Limited. All rights reserved.
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * Licensed under the Apache License, Version 2.0 (the License); you may
 * not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an AS IS BASIS, WITHOUT
 * WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
#define ARMCM55
#if defined (ARMCM55)
  #include "ARMCM55.h"
#else
  #error device not specified!
#endif

/*----------------------------------------------------------------------------
  External References
 *----------------------------------------------------------------------------*/
extern uint32_t __INITIAL_SP;
extern uint32_t __STACK_LIMIT;
#if defined (__ARM_FEATURE_CMSE) && (__ARM_FEATURE_CMSE == 3U)
extern uint32_t __STACK_SEAL;
#endif

extern __NO_RETURN void __PROGRAM_START(void);

/*----------------------------------------------------------------------------
  Internal References
 *----------------------------------------------------------------------------*/
__NO_RETURN void Reset_Handler  (void);
            void Default_Handler(void);

/*----------------------------------------------------------------------------
  Exception / Interrupt Handler
 *----------------------------------------------------------------------------*/
/* Exceptions */
void NMI_Handler            (void) __attribute__ ((weak, alias("Default_Handler")));
extern void HardFault_Handler      (void);
void MemManage_Handler      (void) __attribute__ ((weak, alias("Default_Handler")));
void BusFault_Handler       (void) __attribute__ ((weak, alias("Default_Handler")));
void UsageFault_Handler     (void) __attribute__ ((weak, alias("Default_Handler")));
void SecureFault_Handler    (void) __attribute__ ((weak, alias("Default_Handler")));
void SVC_Handler            (void) __attribute__ ((weak, alias("Default_Handler")));
void DebugMon_Handler       (void) __attribute__ ((weak, alias("Default_Handler")));
extern void PendSV_Handler         (void);
extern void SysTick_Handler        (void);

/*----------------------------------------------------------------------------
  Exception / Interrupt Handler
 *----------------------------------------------------------------------------*/
#define DEFAULT_IRQ_HANDLER(handler_name)  \
void __NO_RETURN __WEAK handler_name(void); \
void handler_name(void) { \
    while(1); \
}

DEFAULT_IRQ_HANDLER(NONSEC_WATCHDOG_RESET_REQ_Handler)
DEFAULT_IRQ_HANDLER(NONSEC_WATCHDOG_Handler)
DEFAULT_IRQ_HANDLER(SLOWCLK_Timer_Handler)
DEFAULT_IRQ_HANDLER(TFM_TIMER0_IRQ_Handler)
DEFAULT_IRQ_HANDLER(TIMER1_Handler)
DEFAULT_IRQ_HANDLER(TIMER2_Handler)
DEFAULT_IRQ_HANDLER(MPC_Handler)
DEFAULT_IRQ_HANDLER(PPC_Handler)
DEFAULT_IRQ_HANDLER(MSC_Handler)
DEFAULT_IRQ_HANDLER(BRIDGE_ERROR_Handler)
DEFAULT_IRQ_HANDLER(MGMT_PPU_Handler)
DEFAULT_IRQ_HANDLER(SYS_PPU_Handler)
DEFAULT_IRQ_HANDLER(CPU0_PPU_Handler)
DEFAULT_IRQ_HANDLER(DEBUG_PPU_Handler)
DEFAULT_IRQ_HANDLER(TIMER3_AON_Handler)
DEFAULT_IRQ_HANDLER(CPU0_CTI_0_Handler)
DEFAULT_IRQ_HANDLER(CPU0_CTI_1_Handler)

DEFAULT_IRQ_HANDLER(System_Timestamp_Counter_Handler)
//DEFAULT_IRQ_HANDLER(UARTRX0_Handler)
extern void UARTRX0_Handler(void);
DEFAULT_IRQ_HANDLER(UARTTX0_Handler)
//DEFAULT_IRQ_HANDLER(UARTRX1_Handler)
extern void UARTRX1_Handler(void);
DEFAULT_IRQ_HANDLER(UARTTX1_Handler)
DEFAULT_IRQ_HANDLER(UARTRX2_Handler)
DEFAULT_IRQ_HANDLER(UARTTX2_Handler)
DEFAULT_IRQ_HANDLER(UARTRX3_Handler)
DEFAULT_IRQ_HANDLER(UARTTX3_Handler)
DEFAULT_IRQ_HANDLER(UARTRX4_Handler)
DEFAULT_IRQ_HANDLER(UARTTX4_Handler)

/*----------------------------------------------------------------------------
  Exception / Interrupt Vector table
 *----------------------------------------------------------------------------*/

#if defined ( __GNUC__ )
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wpedantic"
#endif

extern const VECTOR_TABLE_Type __VECTOR_TABLE[496];
       const VECTOR_TABLE_Type __VECTOR_TABLE[496] __VECTOR_TABLE_ATTRIBUTE = {
  (VECTOR_TABLE_Type)(&__INITIAL_SP),       /*     Initial Stack Pointer */
  Reset_Handler,                            /*     Reset Handler */
  NMI_Handler,                              /* -14 NMI Handler */
  HardFault_Handler,                        /* -13 Hard Fault Handler */
  HardFault_Handler,                        /* -12 MPU Fault Handler */
  HardFault_Handler,                         /* -11 Bus Fault Handler */
  HardFault_Handler,                       /* -10 Usage Fault Handler */
  SecureFault_Handler,                      /*  -9 Secure Fault Handler */
  0,                                        /*     Reserved */
  0,                                        /*     Reserved */
  0,                                        /*     Reserved */
  SVC_Handler,                              /*  -5 SVC Handler */
  DebugMon_Handler,                         /*  -4 Debug Monitor Handler */
  0,                                        /*     Reserved */
  PendSV_Handler,                           /*  -2 PendSV Handler */
  SysTick_Handler,                          /*  -1 SysTick Handler */

  /* Interrupts */
  NONSEC_WATCHDOG_RESET_REQ_Handler, /*   0: Non-Secure Watchdog Reset Request Handler */
  NONSEC_WATCHDOG_Handler,           /*   1: Non-Secure Watchdog Handler */
  SLOWCLK_Timer_Handler,             /*   2: SLOWCLK Timer Handler */
  TFM_TIMER0_IRQ_Handler,            /*   3: TIMER 0 Handler */
  TIMER1_Handler,                    /*   4: TIMER 1 Handler */
  TIMER2_Handler,                    /*   5: TIMER 2 Handler */
  0,                                 /*   6: Reserved */
  0,                                 /*   7: Reserved */
  0,                                 /*   8: Reserved */
  MPC_Handler,                       /*   9: MPC Combined (Secure) Handler */
  PPC_Handler,                       /*  10: PPC Combined (Secure) Handler */
  MSC_Handler,                       /*  11: MSC Combined (Secure) Handler */
  BRIDGE_ERROR_Handler,              /*  12: Bridge Error (Secure) Handler */
  0,                                 /*  13: Reserved */
  MGMT_PPU_Handler,                  /*  14: MGMT PPU Handler */
  SYS_PPU_Handler,                   /*  15: SYS PPU Handler */
  CPU0_PPU_Handler,                  /*  16: CPU0 PPU Handler */
  0,                                 /*  17: Reserved */
  0,                                 /*  18: Reserved */
  0,                                 /*  19: Reserved */
  0,                                 /*  20: Reserved */
  0,                                 /*  21: Reserved */
  0,                                 /*  22: Reserved */
  0,                                 /*  23: Reserved */
  0,                                 /*  24: Reserved */
  0,                                 /*  25: Reserved */
  DEBUG_PPU_Handler,                 /*  26: DEBUG PPU Handler */
  TIMER3_AON_Handler,                /*  27: TIMER 3 AON Handler */
  CPU0_CTI_0_Handler,                /*  28: CPU0 CTI IRQ 0 Handler */
  CPU0_CTI_1_Handler,                /*  29: CPU0 CTI IRQ 1 Handler */
  0,                                 /*  30: Reserved */
  0,                                 /*  31: Reserved */

  /* External interrupts */
  System_Timestamp_Counter_Handler,  /*  32: System timestamp counter Handler */
  UARTRX0_Handler,                   /*  33: UART 0 RX Handler */
  UARTTX0_Handler,                   /*  34: UART 0 TX Handler */
  UARTRX1_Handler,                   /*  35: UART 1 RX Handler */
  UARTTX1_Handler,                   /*  36: UART 1 TX Handler */
  UARTRX2_Handler,                   /*  37: UART 2 RX Handler */
  UARTTX2_Handler,                   /*  38: UART 2 TX Handler */
  UARTRX3_Handler,                   /*  39: UART 3 RX Handler */
  UARTTX3_Handler,                   /*  40: UART 3 TX Handler */
  UARTRX4_Handler,                   /*  41: UART 4 RX Handler */
  UARTTX4_Handler,                   /*  42: UART 4 TX Handler */
  /* Interrupts 37 .. 480 are left out */
};

#if defined ( __GNUC__ )
#pragma GCC diagnostic pop
#endif

/*----------------------------------------------------------------------------
  Reset Handler called on controller reset
 *----------------------------------------------------------------------------*/
__NO_RETURN void Reset_Handler(void)
{
  __set_PSP((uint32_t)(&__INITIAL_SP));

  __set_MSPLIM((uint32_t)(&__STACK_LIMIT));
  __set_PSPLIM((uint32_t)(&__STACK_LIMIT));

#if defined (__ARM_FEATURE_CMSE) && (__ARM_FEATURE_CMSE == 3U)
  __TZ_set_STACKSEAL_S((uint32_t *)(&__STACK_SEAL));
#endif

  SystemInit();                             /* CMSIS System Initialization */
  __PROGRAM_START();                        /* Enter PreMain (C library entry point) */
}


#if defined(__ARMCC_VERSION) && (__ARMCC_VERSION >= 6010050)
  #pragma clang diagnostic push
  #pragma clang diagnostic ignored "-Wmissing-noreturn"
#endif

/*----------------------------------------------------------------------------
  Hard Fault Handler
 *----------------------------------------------------------------------------*/
// void HardFault_Handler(void)
// {
//   while(1);
// }

/*----------------------------------------------------------------------------
  Default Handler for Exceptions / Interrupts
 *----------------------------------------------------------------------------*/
void Default_Handler(void)
{
  while(1);
}

#if defined(__ARMCC_VERSION) && (__ARMCC_VERSION >= 6010050)
  #pragma clang diagnostic pop
#endif

