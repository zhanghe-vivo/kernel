/*
 * Copyright (c) 2020-2024 Arm Limited. All rights reserved.
 *
 * Licensed under the Apache License Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing software
 * distributed under the License is distributed on an "AS IS" BASIS
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

#ifndef __DEVICE_CFG_H__
#define __DEVICE_CFG_H__

/**
 * \file device_cfg.h
 * \brief Configuration file native driver re-targeting
 *
 * \details This file can be used to add native driver specific macro
 *          definitions to select which peripherals are available in the build.
 *
 * This is a default device configuration file with all peripherals enabled.
 */

/* Non Secure only peripheral configuration */

/* ARM MPS3 IO SCC */
// #define MPS3_IO_NS
// #define MPS3_IO_DEV MPS3_IO_DEV_NS

/* I2C_SBCon */
// #define I2C0_SBCON_NS
// #define I2C0_SBCON_DEV I2C0_SBCON_DEV_NS

/* I2S */
// #define MPS3_I2S_NS
// #define MPS3_I2S_DEV MPS3_I2S_DEV_NS

/* ARM UART Controller PL011 */
#define UART0_CMSDK_NS
#define UART0_CMSDK_DEV UART0_CMSDK_DEV_NS
#define UART1_CMSDK_NS
#define UART1_CMSDK_DEV UART1_CMSDK_DEV_NS

#define DEFAULT_UART_BAUDRATE 115200U

/* To be used as CODE and DATA sram */
// #define MPC_ISRAM0_NS
// #define MPC_ISRAM0_DEV MPC_ISRAM0_DEV_NS

// #define MPC_ISRAM1_NS
// #define MPC_ISRAM1_DEV MPC_ISRAM0_DEV_NS

// #define MPC_SRAM_NS
// #define MPC_SRAM_DEV MPC_SRAM_DEV_NS

// #define MPC_QSPI_NS
// #define MPC_QSPI_DEV MPC_QSPI_DEV_NS

/** System Counter Armv8-M */
// #define SYSCOUNTER_CNTRL_ARMV8_M_S
// #define SYSCOUNTER_CNTRL_ARMV8_M_DEV SYSCOUNTER_CNTRL_ARMV8_M_DEV_S

// #define SYSCOUNTER_READ_ARMV8_M_NS
// #define SYSCOUNTER_READ_ARMV8_M_DEV SYSCOUNTER_READ_ARMV8_M_DEV_NS
/**
 * Arbitrary scaling values for test purposes
 */
// #define SYSCOUNTER_ARMV8_M_DEFAULT_SCALE0_INT   1u
// #define SYSCOUNTER_ARMV8_M_DEFAULT_SCALE0_FRACT 0u
// #define SYSCOUNTER_ARMV8_M_DEFAULT_SCALE1_INT   1u
// #define SYSCOUNTER_ARMV8_M_DEFAULT_SCALE1_FRACT 0u

/* System timer */
// #define SYSTIMER0_ARMV8_M_NS
// #define SYSTIMER0_ARMV8_M_DEV SYSTIMER0_ARMV8_M_DEV_NS
// #define SYSTIMER1_ARMV8_M_NS
// #define SYSTIMER1_ARMV8_M_DEV SYSTIMER1_ARMV8_M_DEV_NS
// #define SYSTIMER2_ARMV8_M_NS
// #define SYSTIMER2_ARMV8_M_DEV SYSTIMER2_ARMV8_M_DEV_NS
// #define SYSTIMER3_ARMV8_M_NS
// #define SYSTIMER3_ARMV8_M_DEV SYSTIMER3_ARMV8_M_DEV_NS

// #define SYSTIMER0_ARMV8M_DEFAULT_FREQ_HZ (25000000ul)
// #define SYSTIMER1_ARMV8M_DEFAULT_FREQ_HZ (25000000ul)
// #define SYSTIMER2_ARMV8M_DEFAULT_FREQ_HZ (25000000ul)
// #define SYSTIMER3_ARMV8M_DEFAULT_FREQ_HZ (25000000ul)

/* CMSDK GPIO driver structures */
// #define GPIO0_CMSDK_NS
// #define GPIO0_CMSDK_DEV GPIO0_CMSDK_DEV_NS
// #define GPIO1_CMSDK_NS
// #define GPIO1_CMSDK_DEV GPIO1_CMSDK_DEV_NS
// #define GPIO2_CMSDK_NS
// #define GPIO2_CMSDK_DEV GPIO2_CMSDK_DEV_NS
// #define GPIO3_CMSDK_NS
// #define GPIO3_CMSDK_DEV GPIO3_CMSDK_DEV_NS

/* System Watchdogs */
// #define SYSWDOG_ARMV8_M_NS
// #define SYSWDOG_ARMV8_M_DEV SYSWDOG_ARMV8_M_DEV_NS

/* ARM MPC SIE 300 driver structures */
// #define MPC_VM0_NS
// #define MPC_VM0_DEV MPC_VM0_DEV_NS
// #define MPC_VM1_NS
// #define MPC_VM1_DEV MPC_VM1_DEV_NS
// #define MPC_SSRAM2_NS
// #define MPC_SSRAM2_DEV MPC_SSRAM2_DEV_NS
// #define MPC_SSRAM3_NS
// #define MPC_SSRAM3_DEV MPC_SSRAM3_DEV_NS

/* ARM PPC driver structures */
// #define PPC_SSE300_MAIN0_NS
// #define PPC_SSE300_MAIN0_DEV PPC_SSE300_MAIN0_DEV_NS
// #define PPC_SSE300_MAIN_EXP0_NS
// #define PPC_SSE300_MAIN_EXP0_DEV PPC_SSE300_MAIN_EXP0_DEV_NS
// #define PPC_SSE300_MAIN_EXP1_NS
// #define PPC_SSE300_MAIN_EXP1_DEV PPC_SSE300_MAIN_EXP1_DEV_NS
// #define PPC_SSE300_MAIN_EXP2_NS
// #define PPC_SSE300_MAIN_EXP2_DEV PPC_SSE300_MAIN_EXP2_DEV_NS
// #define PPC_SSE300_MAIN_EXP3_NS
// #define PPC_SSE300_MAIN_EXP3_DEV PPC_SSE300_MAIN_EXP3_DEV_NS
// #define PPC_SSE300_PERIPH0_NS
// #define PPC_SSE300_PERIPH0_DEV PPC_SSE300_PERIPH0_DEV_NS
// #define PPC_SSE300_PERIPH1_NS
// #define PPC_SSE300_PERIPH1_DEV PPC_SSE300_PERIPH1_DEV_NS
// #define PPC_SSE300_PERIPH_EXP0_NS
// #define PPC_SSE300_PERIPH_EXP0_DEV PPC_SSE300_PERIPH_EXP0_DEV_NS
// #define PPC_SSE300_PERIPH_EXP1_NS
// #define PPC_SSE300_PERIPH_EXP1_DEV PPC_SSE300_PERIPH_EXP1_DEV_NS
// #define PPC_SSE300_PERIPH_EXP2_NS
// #define PPC_SSE300_PERIPH_EXP2_DEV PPC_SSE300_PERIPH_EXP2_DEV_NS
// #define PPC_SSE300_PERIPH_EXP3_NS
// #define PPC_SSE300_PERIPH_EXP3_DEV PPC_SSE300_PERIPH_EXP3_DEV_NS

/* ARM SPI PL022 */
/* Invalid device stubs are not defined */
// #define DEFAULT_SPI_SPEED_HZ 4000000U /* 4MHz */
// #define SPI1_PL022_NS
// #define SPI1_PL022_DEV SPI1_PL022_DEV_NS

#endif /* __DEVICE_CFG_H__ */
