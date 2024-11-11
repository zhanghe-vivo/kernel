/*
 * Copyright (c) 2006-2021, RT-Thread Development Team
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * Change Logs:
 * Date           Author       Notes
 * 2018-11-06     balanceTWK   first version
 */

#ifndef __BOARD_H__
#define __BOARD_H__

#ifdef __cplusplus
extern "C" {
#endif

#if defined(__ARMCC_VERSION)
extern int Image$$RW_IRAM1$$ZI$$Limit;
#define HEAP_BEGIN      ((void *)&Image$$RW_IRAM1$$ZI$$Limit)
#elif __ICCARM__
#pragma section="CSTACK"
#define HEAP_BEGIN      (__segment_end("CSTACK"))
#else
extern int __bss_end__;
extern int __HeapLimit;
#define HEAP_BEGIN      ((void *)&__bss_end__)
#endif

#define HEAP_END        ((void *)&__HeapLimit)

void rt_hw_board_init(void);
void rt_hw_systick_init(void);

#ifdef __cplusplus
}
#endif

#endif /* __BOARD_H__ */
