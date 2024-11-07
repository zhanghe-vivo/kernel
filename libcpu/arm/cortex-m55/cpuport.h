/*
 * Copyright (c) 2006-2018, RT-Thread Development Team
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * Change Logs:
 * Date           Author       Notes
 * 2021-04-20     tyx          the first version.
 */

#ifndef __CPUPORT_H__
#define __CPUPORT_H__

#include <rtthread.h>

#if               /* ARMCC */ (  (defined ( __CC_ARM ) && defined ( __TARGET_FPU_VFP ))    \
                  /* Clang */ || (defined ( __clang__ ) && defined ( __VFP_FP__ ) && !defined(__SOFTFP__)) \
                  /* IAR */   || (defined ( __ICCARM__ ) && defined ( __ARMVFP__ ))        \
                  /* GNU */   || (defined ( __GNUC__ ) && defined ( __VFP_FP__ ) && !defined(__SOFTFP__)) )
#define USE_FPU   1
#else
#define USE_FPU   0
#endif

struct exception_stack_frame
{
    rt_uint32_t r0;
    rt_uint32_t r1;
    rt_uint32_t r2;
    rt_uint32_t r3;
    rt_uint32_t r12;
    rt_uint32_t lr;
    rt_uint32_t pc;
    rt_uint32_t psr;
};

struct stack_frame
{
    rt_uint32_t tz;
    rt_uint32_t lr;
    rt_uint32_t psplim;
    rt_uint32_t control;

    /* r4 ~ r11 register */
    rt_uint32_t r4;
    rt_uint32_t r5;
    rt_uint32_t r6;
    rt_uint32_t r7;
    rt_uint32_t r8;
    rt_uint32_t r9;
    rt_uint32_t r10;
    rt_uint32_t r11;

    struct exception_stack_frame exception_stack_frame;
};

struct exception_stack_frame_fpu
{
    rt_uint32_t r0;
    rt_uint32_t r1;
    rt_uint32_t r2;
    rt_uint32_t r3;
    rt_uint32_t r12;
    rt_uint32_t lr;
    rt_uint32_t pc;
    rt_uint32_t psr;

#if USE_FPU
    /* FPU register */
    rt_uint32_t S0;
    rt_uint32_t S1;
    rt_uint32_t S2;
    rt_uint32_t S3;
    rt_uint32_t S4;
    rt_uint32_t S5;
    rt_uint32_t S6;
    rt_uint32_t S7;
    rt_uint32_t S8;
    rt_uint32_t S9;
    rt_uint32_t S10;
    rt_uint32_t S11;
    rt_uint32_t S12;
    rt_uint32_t S13;
    rt_uint32_t S14;
    rt_uint32_t S15;
    rt_uint32_t FPSCR;
    rt_uint32_t NO_NAME;
#endif
};

struct stack_frame_fpu
{
    rt_uint32_t flag;

    /* r4 ~ r11 register */
    rt_uint32_t r4;
    rt_uint32_t r5;
    rt_uint32_t r6;
    rt_uint32_t r7;
    rt_uint32_t r8;
    rt_uint32_t r9;
    rt_uint32_t r10;
    rt_uint32_t r11;

#if USE_FPU
    /* FPU register s16 ~ s31 */
    rt_uint32_t s16;
    rt_uint32_t s17;
    rt_uint32_t s18;
    rt_uint32_t s19;
    rt_uint32_t s20;
    rt_uint32_t s21;
    rt_uint32_t s22;
    rt_uint32_t s23;
    rt_uint32_t s24;
    rt_uint32_t s25;
    rt_uint32_t s26;
    rt_uint32_t s27;
    rt_uint32_t s28;
    rt_uint32_t s29;
    rt_uint32_t s30;
    rt_uint32_t s31;
#endif

    struct exception_stack_frame_fpu exception_stack_frame;
};

struct exception_info
{
    rt_uint32_t exc_return;
    struct stack_frame stack_frame;
};

#endif
