/*
 * Copyright (c) 2006-2021, RT-Thread Development Team
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * Change Logs:
 * Date                 Author             Notes
 */

 #ifndef __RTDEBUG_H__
 #define __RTDEBUG_H__
 
 #include <rtconfig.h>
 /* settings depend check */
 #ifdef RT_USING_POSIX
 #if !defined(RT_USING_DFS) || !defined(RT_USING_DFS_DEVFS)
 #error "POSIX poll/select, stdin need file system(RT_USING_DFS) and device file system(RT_USING_DFS_DEVFS)"
 #endif
 
 #if !defined(RT_USING_LIBC)
 #error "POSIX layer need standard C library(RT_USING_LIBC)"
 #endif
 
 #endif
 
 #ifdef RT_USING_POSIX_TERMIOS
 #if !defined(RT_USING_POSIX)
 #error "termios need POSIX layer(RT_USING_POSIX)"
 #endif
 #endif
 
 /* Using this macro to control all kernel debug features. */
 #ifdef RT_DEBUG
 
 /* Turn on some of these (set to non-zero) to debug kernel */
 #ifndef RT_DEBUG_MEM
 #define RT_DEBUG_MEM                   0
 #endif
 
 #ifndef RT_DEBUG_MEMHEAP
 #define RT_DEBUG_MEMHEAP               0
 #endif
 
 #ifndef RT_DEBUG_MODULE
 #define RT_DEBUG_MODULE                0
 #endif
 
 #ifndef RT_DEBUG_SCHEDULER
 #define RT_DEBUG_SCHEDULER             0
 #endif
 
 #ifndef RT_DEBUG_SLAB
 #define RT_DEBUG_SLAB                  0
 #endif
 
 #ifndef RT_DEBUG_THREAD
 #define RT_DEBUG_THREAD                0
 #endif
 
 #ifndef RT_DEBUG_TIMER
 #define RT_DEBUG_TIMER                 0
 #endif
 
 #ifndef RT_DEBUG_IRQ
 #define RT_DEBUG_IRQ                   0
 #endif
 
 #ifndef RT_DEBUG_IPC
 #define RT_DEBUG_IPC                   0
 #endif
 
 #ifndef RT_DEBUG_DEVICE
 #define RT_DEBUG_DEVICE                0
 #endif
 
 #ifndef RT_DEBUG_INIT
 #define RT_DEBUG_INIT                  0
 #endif
 
 /* Turn on this to enable context check */
 #ifndef RT_DEBUG_CONTEXT_CHECK
 #define RT_DEBUG_CONTEXT_CHECK         1
 #endif
 
 #define RT_DEBUG_LOG(type, message)                                           \
 do                                                                            \
 {                                                                             \
     if (type)                                                                 \
         rt_kprintf message;                                                   \
 }                                                                             \
 while (0)
 
 #define RT_ASSERT(EX)                                                         \
 if (!(EX))                                                                    \
 {                                                                             \
     rt_assert_handler(#EX, __FUNCTION__, __LINE__);                           \
 }
 
 
 #define ASSERT_BUF_LEN 160
 #define RT_ASSERT_EXT(EX, fmt, ...)                                                 \
 do {                                                                                \
     if (!(EX)) {                                                                    \
         char rt_assert_buf[ASSERT_BUF_LEN] = { 0 };                                 \
         int len = rt_snprintf(rt_assert_buf, ASSERT_BUF_LEN, "%s | ", #EX);         \
         rt_snprintf(rt_assert_buf + len, ASSERT_BUF_LEN - len, fmt, ##__VA_ARGS__); \
         rt_assert_handler(rt_assert_buf, __FUNCTION__, __LINE__);                   \
     }                                                                               \
 } while (0)
 
 
 /* Macro to check current context */
 #if RT_DEBUG_CONTEXT_CHECK
 #define RT_DEBUG_NOT_IN_INTERRUPT                                             \
 do                                                                            \
 {                                                                             \
     rt_base_t level;                                                          \
     level = rt_hw_interrupt_disable();                                        \
     if (rt_interrupt_get_nest() != 0)                                         \
     {                                                                         \
         rt_kprintf("Function[%s] shall not be used in ISR\n", __FUNCTION__);  \
         RT_ASSERT_EXT(0, "shall not be used in ISR");                         \
     }                                                                         \
     rt_hw_interrupt_enable(level);                                            \
 }                                                                             \
 while (0)
 
 /* "In thread context" means:
  *     1) the scheduler has been started
  *     2) not in interrupt context.
  */
 #define RT_DEBUG_IN_THREAD_CONTEXT                                            \
 do                                                                            \
 {                                                                             \
     rt_base_t level;                                                          \
     level = rt_hw_interrupt_disable();                                        \
     if (rt_thread_self() == RT_NULL)                                          \
     {                                                                         \
         rt_kprintf("Function[%s] shall not be used before scheduler start\n", \
                    __FUNCTION__);                                             \
         RT_ASSERT_EXT(0, "shall not be used before scheduler start");         \
     }                                                                         \
     RT_DEBUG_NOT_IN_INTERRUPT;                                                \
     rt_hw_interrupt_enable(level);                                            \
 }                                                                             \
 while (0)
 
 /* "scheduler available" means:
  *     1) the scheduler has been started.
  *     2) not in interrupt context.
  *     3) scheduler is not locked.
  *     4) interrupt is not disabled.
  */
 #define RT_DEBUG_SCHEDULER_AVAILABLE(need_check)                              \
 do                                                                            \
 {                                                                             \
     if (need_check)                                                           \
     {                                                                         \
         rt_bool_t interrupt_disabled;                                         \
         rt_base_t level;                                                      \
         interrupt_disabled = rt_hw_interrupt_is_disabled();                   \
         level = rt_hw_interrupt_disable();                                    \
         if (rt_critical_level() != 0)                                         \
         {                                                                     \
             rt_kprintf("Function[%s]: scheduler is not available\n",          \
                     __FUNCTION__);                                            \
             RT_ASSERT_EXT(0, "scheduler is not available");                   \
         }                                                                     \
         if (interrupt_disabled == RT_TRUE)                                    \
         {                                                                     \
             rt_kprintf("Function[%s]: interrupt is disabled\n",               \
                     __FUNCTION__);                                            \
             RT_ASSERT_EXT(0, "interrupt is disabled");                        \
         }                                                                     \
         RT_DEBUG_IN_THREAD_CONTEXT;                                           \
         rt_hw_interrupt_enable(level);                                        \
     }                                                                         \
 }                                                                             \
 while (0)
 #else
 #define RT_DEBUG_NOT_IN_INTERRUPT
 #define RT_DEBUG_IN_THREAD_CONTEXT
 #define RT_DEBUG_SCHEDULER_AVAILABLE(need_check)
 #endif
 
 #else /* RT_DEBUG */
 
 #define RT_ASSERT(EX)
 #define RT_DEBUG_LOG(type, message)
 #define RT_DEBUG_NOT_IN_INTERRUPT
 #define RT_DEBUG_IN_THREAD_CONTEXT
 #define RT_DEBUG_SCHEDULER_AVAILABLE(need_check)
 
 #endif /* RT_DEBUG */
 
 #endif /* __RTDEBUG_H__ */
 