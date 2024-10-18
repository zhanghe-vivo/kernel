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
    rt_thread_info();
    return 0;
}

static void show_wait_queue(struct rt_list_node *list)
{
    struct rt_thread *thread;
    struct rt_list_node *node;

    for (node = list->next; node != list; node = node->next)
    {
        thread = rt_list_entry(node, struct rt_thread, tlist);
        rt_kprintf("%.*s", RT_NAME_MAX, thread->parent.name);

        if (node->next != list)
            rt_kprintf("/");
    }
}

#ifdef RT_USING_SEMAPHORE
long list_sem(void)
{
    //TODO: wait for ipc rewrite

    // rt_base_t level;
    // list_get_next_t find_arg;
    // rt_list_t *obj_list[LIST_FIND_OBJ_NR];
    // rt_list_t *next = (rt_list_t *)RT_NULL;

    // int maxlen;
    // const char *item_title = "semaphore";

    // list_find_init(&find_arg, RT_Object_Class_Semaphore, obj_list, sizeof(obj_list) / sizeof(obj_list[0]));

    // maxlen = RT_NAME_MAX;

    // rt_kprintf("%-*.*s v   suspend thread\n", maxlen, maxlen, item_title);
    // object_split(maxlen);
    // rt_kprintf(" --- --------------\n");

    // do
    // {
    //     next = list_get_next(next, &find_arg);
    //     {
    //         int i;
    //         for (i = 0; i < find_arg.nr_out; i++)
    //         {
    //             struct rt_object *obj;
    //             struct rt_semaphore *sem;

    //             obj = rt_list_entry(obj_list[i], struct rt_object, list);
    //             level = rt_hw_interrupt_disable();
    //             if ((obj->type_ & ~RT_Object_Class_Static) != find_arg.type)
    //             {
    //                 rt_hw_interrupt_enable(level);
    //                 continue;
    //             }
    //             rt_hw_interrupt_enable(level);

    //             sem = (struct rt_semaphore *)obj;
    //             if (!rt_list_isempty(&sem->parent.suspend_thread))
    //             {
    //                 rt_kprintf("%-*.*s %03d %d:",
    //                            maxlen, RT_NAME_MAX,
    //                            sem->parent.parent.name,
    //                            sem->value,
    //                            rt_list_len(&sem->parent.suspend_thread));
    //                 show_wait_queue(&(sem->parent.suspend_thread));
    //                 rt_kprintf("\n");
    //             }
    //             else
    //             {
    //                 rt_kprintf("%-*.*s %03d %d\n",
    //                            maxlen, RT_NAME_MAX,
    //                            sem->parent.parent.name,
    //                            sem->value,
    //                            rt_list_len(&sem->parent.suspend_thread));
    //             }
    //         }
    //     }
    // }
    // while (next != (rt_list_t *)RT_NULL);

    return 0;
}
#endif /* RT_USING_SEMAPHORE */

#ifdef RT_USING_EVENT
long list_event(void)
{
    // rt_base_t level;
    // list_get_next_t find_arg;
    // rt_list_t *obj_list[LIST_FIND_OBJ_NR];
    // rt_list_t *next = (rt_list_t *)RT_NULL;

    // int maxlen;
    // const char *item_title = "event";

    // list_find_init(&find_arg, RT_Object_Class_Event, obj_list, sizeof(obj_list) / sizeof(obj_list[0]));

    // maxlen = RT_NAME_MAX;

    // rt_kprintf("%-*.*s      set    suspend thread\n", maxlen, maxlen, item_title);
    // object_split(maxlen);
    // rt_kprintf("  ---------- --------------\n");

    // do
    // {
    //     next = list_get_next(next, &find_arg);
    //     {
    //         int i;
    //         for (i = 0; i < find_arg.nr_out; i++)
    //         {
    //             struct rt_object *obj;
    //             struct rt_event *e;

    //             obj = rt_list_entry(obj_list[i], struct rt_object, list);
    //             level = rt_hw_interrupt_disable();
    //             if ((obj->type_ & ~RT_Object_Class_Static) != find_arg.type)
    //             {
    //                 rt_hw_interrupt_enable(level);
    //                 continue;
    //             }

    //             rt_hw_interrupt_enable(level);

    //             e = (struct rt_event *)obj;
    //             if (!rt_list_isempty(&e->parent.suspend_thread))
    //             {
    //                 rt_kprintf("%-*.*s  0x%08x %03d:",
    //                            maxlen, RT_NAME_MAX,
    //                            e->parent.parent.name,
    //                            e->set,
    //                            rt_list_len(&e->parent.suspend_thread));
    //                 show_wait_queue(&(e->parent.suspend_thread));
    //                 rt_kprintf("\n");
    //             }
    //             else
    //             {
    //                 rt_kprintf("%-*.*s  0x%08x 0\n",
    //                            maxlen, RT_NAME_MAX, e->parent.parent.name, e->set);
    //             }
    //         }
    //     }
    // }
    // while (next != (rt_list_t *)RT_NULL);

    return 0;
}
#endif /* RT_USING_EVENT */

#ifdef RT_USING_MUTEX
long list_mutex(void)
{
    // rt_base_t level;
    // list_get_next_t find_arg;
    // rt_list_t *obj_list[LIST_FIND_OBJ_NR];
    // rt_list_t *next = (rt_list_t *)RT_NULL;

    // int maxlen;
    // const char *item_title = "mutex";

    // list_find_init(&find_arg, RT_Object_Class_Mutex, obj_list, sizeof(obj_list) / sizeof(obj_list[0]));

    // maxlen = RT_NAME_MAX;

    // rt_kprintf("%-*.*s   owner  hold priority suspend thread \n", maxlen, maxlen, item_title);
    // object_split(maxlen);
    // rt_kprintf(" -------- ---- -------- --------------\n");

    // do
    // {
    //     next = list_get_next(next, &find_arg);
    //     {
    //         int i;
    //         for (i = 0; i < find_arg.nr_out; i++)
    //         {
    //             struct rt_object *obj;
    //             struct rt_mutex *m;

    //             obj = rt_list_entry(obj_list[i], struct rt_object, list);
    //             level = rt_hw_interrupt_disable();
    //             if ((obj->type_ & ~RT_Object_Class_Static) != find_arg.type)
    //             {
    //                 rt_hw_interrupt_enable(level);
    //                 continue;
    //             }

    //             rt_hw_interrupt_enable(level);

    //             m = (struct rt_mutex *)obj;
    //             if (!rt_list_isempty(&m->parent.suspend_thread))
    //             {
    //                 rt_kprintf("%-*.*s %-8.*s %04d %8d  %04d ",
    //                        maxlen, RT_NAME_MAX,
    //                        m->parent.parent.name,
    //                        RT_NAME_MAX,
    //                        m->owner->parent.name,
    //                        m->hold,
    //                        m->priority,
    //                        rt_list_len(&m->parent.suspend_thread));
    //                 show_wait_queue(&(m->parent.suspend_thread));
    //                 rt_kprintf("\n");
    //             }
    //             else
    //             {
    //                 rt_kprintf("%-*.*s %-8.*s %04d %8d  %04d\n",
    //                        maxlen, RT_NAME_MAX,
    //                        m->parent.parent.name,
    //                        RT_NAME_MAX,
    //                        m->owner->parent.name,
    //                        m->hold,
    //                        m->priority,
    //                        rt_list_len(&m->parent.suspend_thread));
    //             }
    //         }
    //     }
    // }
    // while (next != (rt_list_t *)RT_NULL);

    return 0;
}
#endif /* RT_USING_MUTEX */

#ifdef RT_USING_MAILBOX
long list_mailbox(void)
{
    // rt_base_t level;
    // list_get_next_t find_arg;
    // rt_list_t *obj_list[LIST_FIND_OBJ_NR];
    // rt_list_t *next = (rt_list_t *)RT_NULL;

    // int maxlen;
    // const char *item_title = "mailbox";

    // list_find_init(&find_arg, RT_Object_Class_MailBox, obj_list, sizeof(obj_list) / sizeof(obj_list[0]));

    // maxlen = RT_NAME_MAX;

    // rt_kprintf("%-*.*s entry size suspend thread\n", maxlen, maxlen, item_title);
    // object_split(maxlen);
    // rt_kprintf(" ----  ---- --------------\n");

    // do
    // {
    //     next = list_get_next(next, &find_arg);
    //     {
    //         int i;
    //         for (i = 0; i < find_arg.nr_out; i++)
    //         {
    //             struct rt_object *obj;
    //             struct rt_mailbox *m;

    //             obj = rt_list_entry(obj_list[i], struct rt_object, list);
    //             level = rt_hw_interrupt_disable();
    //             if ((obj->type_ & ~RT_Object_Class_Static) != find_arg.type)
    //             {
    //                 rt_hw_interrupt_enable(level);
    //                 continue;
    //             }

    //             rt_hw_interrupt_enable(level);

    //             m = (struct rt_mailbox *)obj;
    //             if (!rt_list_isempty(&m->parent.suspend_thread))
    //             {
    //                 rt_kprintf("%-*.*s %04d  %04d %d:",
    //                            maxlen, RT_NAME_MAX,
    //                            m->parent.parent.name,
    //                            m->entry,
    //                            m->size,
    //                            rt_list_len(&m->parent.suspend_thread));
    //                 show_wait_queue(&(m->parent.suspend_thread));
    //                 rt_kprintf("\n");
    //             }
    //             else
    //             {
    //                 rt_kprintf("%-*.*s %04d  %04d %d\n",
    //                            maxlen, RT_NAME_MAX,
    //                            m->parent.parent.name,
    //                            m->entry,
    //                            m->size,
    //                            rt_list_len(&m->parent.suspend_thread));
    //             }

    //         }
    //     }
    // }
    // while (next != (rt_list_t *)RT_NULL);

    return 0;
}
#endif /* RT_USING_MAILBOX */

#ifdef RT_USING_MESSAGEQUEUE
long list_msgqueue(void)
{
    // rt_base_t level;
    // list_get_next_t find_arg;
    // rt_list_t *obj_list[LIST_FIND_OBJ_NR];
    // rt_list_t *next = (rt_list_t *)RT_NULL;

    // int maxlen;
    // const char *item_title = "msgqueue";

    // list_find_init(&find_arg, RT_Object_Class_MessageQueue, obj_list, sizeof(obj_list) / sizeof(obj_list[0]));

    // maxlen = RT_NAME_MAX;

    // rt_kprintf("%-*.*s entry suspend thread\n", maxlen, maxlen, item_title);
    // object_split(maxlen);
    // rt_kprintf(" ----  --------------\n");
    // do
    // {
    //     next = list_get_next(next, &find_arg);
    //     {
    //         int i;
    //         for (i = 0; i < find_arg.nr_out; i++)
    //         {
    //             struct rt_object *obj;
    //             struct rt_messagequeue *m;

    //             obj = rt_list_entry(obj_list[i], struct rt_object, list);
    //             level = rt_hw_interrupt_disable();
    //             if ((obj->type_ & ~RT_Object_Class_Static) != find_arg.type)
    //             {
    //                 rt_hw_interrupt_enable(level);
    //                 continue;
    //             }

    //             rt_hw_interrupt_enable(level);

    //             m = (struct rt_messagequeue *)obj;
    //             if (!rt_list_isempty(&m->parent.suspend_thread))
    //             {
    //                 rt_kprintf("%-*.*s %04d  %d:",
    //                            maxlen, RT_NAME_MAX,
    //                            m->parent.parent.name,
    //                            m->entry,
    //                            rt_list_len(&m->parent.suspend_thread));
    //                 show_wait_queue(&(m->parent.suspend_thread));
    //                 rt_kprintf("\n");
    //             }
    //             else
    //             {
    //                 rt_kprintf("%-*.*s %04d  %d\n",
    //                            maxlen, RT_NAME_MAX,
    //                            m->parent.parent.name,
    //                            m->entry,
    //                            rt_list_len(&m->parent.suspend_thread));
    //             }
    //         }
    //     }
    // }
    // while (next != (rt_list_t *)RT_NULL);

    return 0;
}
#endif /* RT_USING_MESSAGEQUEUE */

#ifdef RT_USING_MEMHEAP
long list_memheap(void)
{
    // rt_base_t level;
    // list_get_next_t find_arg;
    // rt_list_t *obj_list[LIST_FIND_OBJ_NR];
    // rt_list_t *next = (rt_list_t *)RT_NULL;

    // int maxlen;
    // const char *item_title = "memheap";

    // list_find_init(&find_arg, RT_Object_Class_MemHeap, obj_list, sizeof(obj_list) / sizeof(obj_list[0]));

    // maxlen = RT_NAME_MAX;

    // rt_kprintf("%-*.*s  pool size  max used size available size\n", maxlen, maxlen, item_title);
    // object_split(maxlen);
    // rt_kprintf(" ---------- ------------- --------------\n");
    // do
    // {
    //     next = list_get_next(next, &find_arg);
    //     {
    //         int i;
    //         for (i = 0; i < find_arg.nr_out; i++)
    //         {
    //             struct rt_object *obj;
    //             struct rt_memheap *mh;

    //             obj = rt_list_entry(obj_list[i], struct rt_object, list);
    //             level = rt_hw_interrupt_disable();
    //             if ((obj->type_ & ~RT_Object_Class_Static) != find_arg.type)
    //             {
    //                 rt_hw_interrupt_enable(level);
    //                 continue;
    //             }

    //             rt_hw_interrupt_enable(level);

    //             mh = (struct rt_memheap *)obj;

    //             rt_kprintf("%-*.*s %-010d %-013d %-05d\n",
    //                        maxlen, RT_NAME_MAX,
    //                        mh->parent.name,
    //                        mh->pool_size,
    //                        mh->max_used_size,
    //                        mh->available_size);

    //         }
    //     }
    // }
    // while (next != (rt_list_t *)RT_NULL);

    return 0;
}
#endif /* RT_USING_MEMHEAP */

#ifdef RT_USING_MEMPOOL
long list_mempool(void)
{
    // rt_base_t level;
    // list_get_next_t find_arg;
    // rt_list_t *obj_list[LIST_FIND_OBJ_NR];
    // rt_list_t *next = (rt_list_t *)RT_NULL;

    // int maxlen;
    // const char *item_title = "mempool";

    // list_find_init(&find_arg, RT_Object_Class_MemPool, obj_list, sizeof(obj_list) / sizeof(obj_list[0]));

    // maxlen = RT_NAME_MAX;

    // rt_kprintf("%-*.*s block total free suspend thread\n", maxlen, maxlen, item_title);
    // object_split(maxlen);
    // rt_kprintf(" ----  ----  ---- --------------\n");
    // do
    // {
    //     next = list_get_next(next, &find_arg);
    //     {
    //         int i;
    //         for (i = 0; i < find_arg.nr_out; i++)
    //         {
    //             struct rt_object *obj;
    //             struct rt_mempool *mp;
    //             int suspend_thread_count;
    //             rt_list_t *node;

    //             obj = rt_list_entry(obj_list[i], struct rt_object, list);
    //             level = rt_hw_interrupt_disable();
    //             if ((obj->type_ & ~RT_Object_Class_Static) != find_arg.type)
    //             {
    //                 rt_hw_interrupt_enable(level);
    //                 continue;
    //             }

    //             rt_hw_interrupt_enable(level);

    //             mp = (struct rt_mempool *)obj;

    //             suspend_thread_count = 0;
    //             rt_list_for_each(node, &mp->suspend_thread)
    //             {
    //                 suspend_thread_count++;
    //             }

    //             if (suspend_thread_count > 0)
    //             {
    //                 rt_kprintf("%-*.*s %04d  %04d  %04d %d:",
    //                            maxlen, RT_NAME_MAX,
    //                            mp->parent.name,
    //                            mp->block_size,
    //                            mp->block_total_count,
    //                            mp->block_free_count,
    //                            suspend_thread_count);
    //                 show_wait_queue(&(mp->suspend_thread));
    //                 rt_kprintf("\n");
    //             }
    //             else
    //             {
    //                 rt_kprintf("%-*.*s %04d  %04d  %04d %d\n",
    //                            maxlen, RT_NAME_MAX,
    //                            mp->parent.name,
    //                            mp->block_size,
    //                            mp->block_total_count,
    //                            mp->block_free_count,
    //                            suspend_thread_count);
    //             }
    //         }
    //     }
    // }
    // while (next != (rt_list_t *)RT_NULL);

    return 0;
}
#endif /* RT_USING_MEMPOOL */

long list_timer(void)
{
    rt_timer_info();
    return 0;
}

void rt_print_name(const char *name){
    rt_kprintf("%s",name);
    rt_kprintf("%p",name);
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
    // rt_base_t level;
    // list_get_next_t find_arg;
    // rt_list_t *obj_list[LIST_FIND_OBJ_NR];
    // rt_list_t *next = (rt_list_t *)RT_NULL;
    // const char *device_type;

    // int maxlen;
    // const char *item_title = "device";

    // list_find_init(&find_arg, RT_Object_Class_Device, obj_list, sizeof(obj_list) / sizeof(obj_list[0]));

    // maxlen = RT_NAME_MAX;

    // rt_kprintf("%-*.*s         type         ref count\n", maxlen, maxlen, item_title);
    // object_split(maxlen);
    // rt_kprintf(" -------------------- ----------\n");
    // do
    // {
    //     next = list_get_next(next, &find_arg);
    //     {
    //         int i;
    //         for (i = 0; i < find_arg.nr_out; i++)
    //         {
    //             struct rt_object *obj;
    //             struct rt_device *device;

    //             obj = rt_list_entry(obj_list[i], struct rt_object, list);
    //             level = rt_hw_interrupt_disable();
    //             if ((obj->type_ & ~RT_Object_Class_Static) != find_arg.type)
    //             {
    //                 rt_hw_interrupt_enable(level);
    //                 continue;
    //             }

    //             rt_hw_interrupt_enable(level);

    //             device = (struct rt_device *)obj;
    //             device_type = "Unknown";
    //             if (device->type < RT_Device_Class_Unknown &&
    //                 device_type_str[device->type] != RT_NULL)
    //             {
    //                 device_type = device_type_str[device->type];
    //             }
    //             rt_kprintf("%-*.*s %-20s %-8d\n",
    //                        maxlen, RT_NAME_MAX,
    //                        device->parent.name,
    //                        device_type,
    //                        device->ref_count);

    //         }
    //     }
    // }
    // while (next != (rt_list_t *)RT_NULL);

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
