/*
 * Copyright (c) 2006-2022, RT-Thread Development Team
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * Change Logs:
 * Date           Author       Notes
 * 2006-04-30     Bernard      first implementation
 * 2006-05-04     Bernard      add list_thread,
 *                                 list_sem,
 *                                 list_timer
 * 2006-05-20     Bernard      add list_mutex,
 *                                 list_mailbox,
 *                                 list_msgqueue,
 *                                 list_event,
 *                                 list_fevent,
 *                                 list_mempool
 * 2006-06-03     Bernard      display stack information in list_thread
 * 2006-08-10     Bernard      change version to invoke rt_show_version
 * 2008-09-10     Bernard      update the list function for finsh syscall
 *                                 list and sysvar list
 * 2009-05-30     Bernard      add list_device
 * 2010-04-21     yi.qiu       add list_module
 * 2012-04-29     goprife      improve the command line auto-complete feature.
 * 2012-06-02     lgnq         add list_memheap
 * 2012-10-22     Bernard      add MS VC++ patch.
 * 2016-06-02     armink       beautify the list_thread command
 * 2018-11-22     Jesven       list_thread add smp support
 * 2018-12-27     Jesven       Fix the problem that disable interrupt too long in list_thread
 *                             Provide protection for the "first layer of objects" when list_*
 * 2020-04-07     chenhui      add clear
 * 2022-07-02     Stanley Lwin add list command
 */

#include <rthw.h>
#include <rtthread.h>
#include <string.h>

#ifdef RT_USING_FINSH
#include <finsh.h>

#define LIST_DFS_OPT_ID 0x100
#define LIST_FIND_OBJ_NR 8

static long clear(void)
{
    rt_kprintf("\x1b[2J\x1b[H");

    return 0;
}
MSH_CMD_EXPORT(clear, clear the terminal screen);

extern void rt_show_version(void);
long version(void)
{
    rt_show_version();

    return 0;
}
MSH_CMD_EXPORT(version, show RT-Thread version information);


long list_thread(void)
{
    //TODO: rewrite rust thread info
    return 0;
}

#ifdef RT_USING_SEMAPHORE
long list_sem(void)
{
    //TODO: rewrite rust semaphore info
    return 0;
}
#endif /* RT_USING_SEMAPHORE */

#ifdef RT_USING_EVENT
long list_event(void)
{
    //TODO: rewrite rust event info
    return 0;
}
#endif /* RT_USING_EVENT */

#ifdef RT_USING_MUTEX
long list_mutex(void)
{
    //TODO: rewrite rust mutex info
    return 0;
}
#endif /* RT_USING_MUTEX */

#ifdef RT_USING_MAILBOX
long list_mailbox(void)
{
    //TODO: rewrite rust mailbox info
    return 0;
}
#endif /* RT_USING_MAILBOX */

#ifdef RT_USING_MESSAGEQUEUE
long list_msgqueue(void)
{
    //TODO: rewrite rust msgqueue info
    return 0;
}
#endif /* RT_USING_MESSAGEQUEUE */

#ifdef RT_USING_MEMHEAP
long list_memheap(void)
{
    //TODO: rewrite rust memheap

    return 0;
}
#endif /* RT_USING_MEMHEAP */

#ifdef RT_USING_MEMPOOL
long list_mempool(void)
{
    //TODO: rewrite rust mempool

    return 0;
}
#endif /* RT_USING_MEMPOOL */

long list_timer(void)
{
    //TODO: rewrite rust timer info
    return 0;
}

#ifdef RT_USING_DEVICE
static char *const device_type_str[RT_Device_Class_Unknown] =
{
    "Character Device",
    "Block Device",
    "Network Interface",
    "MTD Device",
    "CAN Device",
    "RTC",
    "Sound Device",
    "Graphic Device",
    "I2C Bus",
    "USB Slave Device",
    "USB Host Bus",
    "USB OTG Bus",
    "SPI Bus",
    "SPI Device",
    "SDIO Bus",
    "PM Pseudo Device",
    "Pipe",
    "Portal Device",
    "Timer Device",
    "Miscellaneous Device",
    "Sensor Device",
    "Touch Device",
    "Phy Device",
    "Security Device",
    "WLAN Device",
    "Pin Device",
    "ADC Device",
    "DAC Device",
    "WDT Device",
    "PWM Device",
    "Bus Device",
};

long list_device(void)
{
    //TODO: rewrite rust device
    return 0;
}
#endif /* RT_USING_DEVICE */

#ifndef FINSH_USING_OPTION_COMPLETION
int cmd_list(int argc, char **argv)
{
    if(argc == 2)
    {
        if(strcmp(argv[1], "thread") == 0)
        {
            list_thread();
        }
        else if(strcmp(argv[1], "timer") == 0)
        {
            list_timer();
        }
#ifdef RT_USING_SEMAPHORE
        else if(strcmp(argv[1], "sem") == 0)
        {
            list_sem();
        }
#endif /* RT_USING_SEMAPHORE */
#ifdef RT_USING_EVENT
        else if(strcmp(argv[1], "event") == 0)
        {
            list_event();
        }
#endif /* RT_USING_EVENT */
#ifdef RT_USING_MUTEX
        else if(strcmp(argv[1], "mutex") == 0)
        {
            list_mutex();
        }
#endif /* RT_USING_MUTEX */
#ifdef RT_USING_MAILBOX
        else if(strcmp(argv[1], "mailbox") == 0)
        {
            list_mailbox();
        }
#endif  /* RT_USING_MAILBOX */
#ifdef RT_USING_MESSAGEQUEUE
        else if(strcmp(argv[1], "msgqueue") == 0)
        {
            list_msgqueue();
        }
#endif /* RT_USING_MESSAGEQUEUE */
#ifdef RT_USING_MEMHEAP
        else if(strcmp(argv[1], "memheap") == 0)
        {
            list_memheap();
        }
#endif /* RT_USING_MEMHEAP */
#ifdef RT_USING_MEMPOOL
        else if(strcmp(argv[1], "mempool") == 0)
        {
            list_mempool();
        }
#endif /* RT_USING_MEMPOOL */
#ifdef RT_USING_DEVICE
        else if(strcmp(argv[1], "device") == 0)
        {
            list_device();
        }
#endif /* RT_USING_DEVICE */
#ifdef RT_USING_DFS
        else if(strcmp(argv[1], "fd") == 0)
        {
            extern int list_fd(void);
            list_fd();
        }
#endif /* RT_USING_DFS */
        else
        {
            goto _usage;
        }

        return 0;
    }

_usage:
    rt_kprintf("Usage: list [options]\n");
    rt_kprintf("[options]:\n");
    rt_kprintf("    %-12s - list threads\n", "thread");
    rt_kprintf("    %-12s - list timers\n", "timer");
#ifdef RT_USING_SEMAPHORE
    rt_kprintf("    %-12s - list semaphores\n", "sem");
#endif /* RT_USING_SEMAPHORE */
#ifdef RT_USING_MUTEX
    rt_kprintf("    %-12s - list mutexs\n", "mutex");
#endif /* RT_USING_MUTEX */
#ifdef RT_USING_EVENT
    rt_kprintf("    %-12s - list events\n", "event");
#endif /* RT_USING_EVENT */
#ifdef RT_USING_MAILBOX
    rt_kprintf("    %-12s - list mailboxs\n", "mailbox");
#endif /* RT_USING_MAILBOX */
#ifdef RT_USING_MESSAGEQUEUE
    rt_kprintf("    %-12s - list message queues\n", "msgqueue");
#endif /* RT_USING_MESSAGEQUEUE */
#ifdef RT_USING_MEMHEAP
    rt_kprintf("    %-12s - list memory heaps\n", "memheap");
#endif /* RT_USING_MEMHEAP */
#ifdef RT_USING_MEMPOOL
    rt_kprintf("    %-12s - list memory pools\n", "mempool");
#endif /* RT_USING_MEMPOOL */
#ifdef RT_USING_DEVICE
    rt_kprintf("    %-12s - list devices\n", "device");
#endif /* RT_USING_DEVICE */
#ifdef RT_USING_DFS
    rt_kprintf("    %-12s - list file descriptors\n", "fd");
#endif /* RT_USING_DFS */

    return 0;
}

#else
CMD_OPTIONS_STATEMENT(cmd_list)
int cmd_list(int argc, char **argv)
{
    if (argc == 2)
    {
        switch (MSH_OPT_ID_GET(cmd_list))
        {
        case RT_Object_Class_Thread: list_thread(); break;
        case RT_Object_Class_Timer: list_timer(); break;
#ifdef RT_USING_SEMAPHORE
        case RT_Object_Class_Semaphore: list_sem(); break;
#endif /* RT_USING_SEMAPHORE */
#ifdef RT_USING_EVENT
        case RT_Object_Class_Event: list_event(); break;
#endif /* RT_USING_EVENT */
#ifdef RT_USING_MUTEX
        case RT_Object_Class_Mutex: list_mutex(); break;
#endif /* RT_USING_MUTEX */
#ifdef RT_USING_MAILBOX
        case RT_Object_Class_MailBox: list_mailbox(); break;
#endif  /* RT_USING_MAILBOX */
#ifdef RT_USING_MESSAGEQUEUE
        case RT_Object_Class_MessageQueue: list_msgqueue(); break;
#endif /* RT_USING_MESSAGEQUEUE */
#ifdef RT_USING_MEMHEAP
        case RT_Object_Class_MemHeap: list_memheap(); break;
#endif /* RT_USING_MEMHEAP */
#ifdef RT_USING_MEMPOOL
        case RT_Object_Class_MemPool: list_mempool(); break;
#endif /* RT_USING_MEMPOOL */
#ifdef RT_USING_DEVICE
        case RT_Object_Class_Device: list_device(); break;
#endif /* RT_USING_DEVICE */
#ifdef RT_USING_DFS
        case LIST_DFS_OPT_ID:
        {
            extern int list_fd(void);
            list_fd();
            break;
        }
#endif /* RT_USING_DFS */
        default:
            goto _usage;
            break;
        };

        return 0;
        }

_usage:
    rt_kprintf("Usage: list [options]\n");
    rt_kprintf("[options]:\n");
    MSH_OPT_DUMP(cmd_list);
    return 0;
}
CMD_OPTIONS_NODE_START(cmd_list)
CMD_OPTIONS_NODE(RT_Object_Class_Thread,       thread,   list threads)
CMD_OPTIONS_NODE(RT_Object_Class_Timer,        timer,    list timers)
#ifdef RT_USING_SEMAPHORE
CMD_OPTIONS_NODE(RT_Object_Class_Semaphore,    sem,      list semaphores)
#endif /* RT_USING_SEMAPHORE */
#ifdef RT_USING_EVENT
CMD_OPTIONS_NODE(RT_Object_Class_Event,        event,    list events)
#endif /* RT_USING_EVENT */
#ifdef RT_USING_MUTEX
CMD_OPTIONS_NODE(RT_Object_Class_Mutex,        mutex,    list mutexs)
#endif /* RT_USING_MUTEX */
#ifdef RT_USING_MAILBOX
CMD_OPTIONS_NODE(RT_Object_Class_MailBox,      mailbox,  list mailboxs)
#endif  /* RT_USING_MAILBOX */
#ifdef RT_USING_MESSAGEQUEUE
CMD_OPTIONS_NODE(RT_Object_Class_MessageQueue, msgqueue, list message queues)
#endif /* RT_USING_MESSAGEQUEUE */
#ifdef RT_USING_MEMHEAP
CMD_OPTIONS_NODE(RT_Object_Class_MemHeap,      memheap,  list memory heaps)
#endif /* RT_USING_MEMHEAP */
#ifdef RT_USING_MEMPOOL
CMD_OPTIONS_NODE(RT_Object_Class_MemPool,      mempool,  list memory pools)
#endif /* RT_USING_MEMPOOL */
#ifdef RT_USING_DEVICE
CMD_OPTIONS_NODE(RT_Object_Class_Device,       device,   list devices)
#endif /* RT_USING_DEVICE */
#ifdef RT_USING_DFS
CMD_OPTIONS_NODE(LIST_DFS_OPT_ID,              fd,       list file descriptors)
#endif /* RT_USING_DFS */
CMD_OPTIONS_NODE_END
#endif /* FINSH_USING_OPTION_COMPLETION */
MSH_CMD_EXPORT_ALIAS(cmd_list, list, list objects, optenable);

#endif /* RT_USING_FINSH */
