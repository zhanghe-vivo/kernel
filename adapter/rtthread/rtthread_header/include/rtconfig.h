/*
 * Copyright (c) 2006-2022, RT-Thread Development Team
 *
 * SPDX-License-Identifier: Apache-2.0
 */

#define RT_USING_NEWLIB

/* RT-Thread Kernel */

#define RT_NAME_MAX 8
#define RT_ALIGN_SIZE 4
#define RT_THREAD_PRIORITY_32
#define RT_THREAD_PRIORITY_MAX 32
#define RT_TICK_PER_SECOND 1024
#define RT_USING_OVERFLOW_CHECK
#define RT_STACK_WARNING_THRESHOLD 80
#define RT_OVERFLOW_CHECK_HARDWARE
#define RT_USING_HOOK
#define RT_HOOK_USING_FUNC_PTR
#define RT_USING_PRIOR_INVER_OPT
#define MAX_HASH_THREAD_NUM 128
#define MAX_MUTEX_NUM 10
#define RT_USING_IDLE_HOOK
#define RT_IDLE_HOOK_LIST_SIZE 4
#define IDLE_THREAD_STACK_SIZE 4096
#define RT_USING_TIMER_SOFT
#define RT_TIMER_THREAD_PRIO 1
#define RT_TIMER_THREAD_STACK_SIZE 4096
#define RT_TIMER_SOFT_LPTMR
#define RT_USING_THREAD_MONITOR
#define RT_USING_CPU_USAGE
#define RT_CPU_USAGE_TIME_UNIT 1000
#define RT_CPU_USAGE_PRECISION_MULTIPLE 1000

/* kservice optimization */

/* end of kservice optimization */
#define RT_DEBUG
#define RT_DEBUG_CONTEXT_CHECK 0
#define INT_BASEPRI_DEBUG
#define VS_MEMPOOL_DEBUG
#define VS_INTERRUPT_DEBUG
#define VS_SYS_THREAD_CHECK

/* Inter-Thread communication */

#define RT_USING_SEMAPHORE
#define RT_USING_MUTEX
#define RT_USING_DEADLOCK_CHECK
#define RT_USING_EVENT
#define RT_USING_MAILBOX
#define RT_USING_MESSAGEQUEUE
#define RT_USING_WORKQUEUE
#define RT_USING_COMPLETION
#define RT_USING_CRC32
/* end of Inter-Thread communication */

/* Memory Management */

#define RT_USING_MEMPOOL
#define RT_MEMPOOL_TAIL_INSERT
#define RT_MEMPOOL_USAGE_STATISTICS
#define RT_USING_MEMPOOL_DEBUG
#define RT_USING_MEMHEAP
#define RT_USING_MEMHEAP_AS_HEAP
#define RT_USING_MEMTRACE
#define RT_MEMHEAP_BEST_FIT
#define RT_USING_PMEM
#define RT_USING_HEAP
#define RT_USING_MEM_LEAK_CHECK
#define RT_MEM_LEAK_CHECK_FULL
#define SRAM_CP_HEAP_ENABLE
/* end of Memory Management */

/* Kernel Device Object */

#define RT_USING_DEVICE
#define RT_USING_CONSOLE
#define RT_CONSOLEBUF_SIZE 256
#define RT_CONSOLE_DEVICE_NAME "console"
/* end of Kernel Device Object */