#include <rtthread.h>
#include "utest.h"
#include <stdlib.h>
#include <stdbool.h>


static struct rt_condvar static_condvar = {0};
static struct rt_mutex static_mutex = {0};

static rt_uint32_t condition = 2;
static rt_uint32_t static_condvar_wait_thread_finish = 0, static_condvar_notify_thread_finish = 0;

rt_align(RT_ALIGN_SIZE)
static char thread1_stack[UTEST_THR_STACK_SIZE];
static struct rt_thread thread1;

rt_align(RT_ALIGN_SIZE)
static char thread2_stack[UTEST_THR_STACK_SIZE];
static struct rt_thread thread2;

#define THREAD_PRIORITY      9
#define THREAD_TIMESLICE     5

static void test_condvar_init(void)
{
    rt_err_t result;


    result = rt_condvar_init(&static_condvar, "condvar", RT_IPC_FLAG_PRIO);
    if (result != RT_EOK)
    {
        uassert_false(1);
    }
    result = rt_condvar_detach(&static_condvar);
    if (result != RT_EOK)
    {
        uassert_false(1);
    }

    result = rt_condvar_init(&static_condvar, "condvar", RT_IPC_FLAG_FIFO);
    if (result != RT_EOK)
    {
        uassert_false(1);
    }
    result = rt_condvar_detach(&static_condvar);
    if (result != RT_EOK)
    {
        uassert_false(1);
    }

    uassert_true(1);
}

static void test_condvar_detach(void)
{
    rt_err_t result = RT_EOK;

    result = rt_condvar_init(&static_condvar, "condvar", RT_IPC_FLAG_PRIO);
    if (result != RT_EOK)
    {
        uassert_false(1);
    }

    result = rt_condvar_detach(&static_condvar);
    if (result != RT_EOK)
    {
        uassert_false(1);
    }

    uassert_true(1);
}

static void thread1_condvar_wait(void *param)
{
    rt_err_t ret = RT_EOK;

    while (true) {
        ret = rt_mutex_take(&static_mutex, RT_WAITING_FOREVER);
        if (ret != RT_EOK)
        {
            uassert_false(1);
        }
        if (rt_condvar_wait(&static_condvar, &static_mutex) == RT_EOK )
        {
            if (condition == 0) static_condvar_wait_thread_finish = 1;
            ret = rt_mutex_release(&static_mutex);
            if (static_condvar_wait_thread_finish)
                break;

        } else
        {
            uassert_false(1);
        }
    }
}

static void thread2_notify_condvar(void *param)
{
    rt_err_t ret = RT_EOK;

    while (true) {
        ret = rt_mutex_take(&static_mutex, RT_WAITING_FOREVER);
        if (ret != RT_EOK)
        {
            uassert_false(1);
        }

        if (condition == 2) {
            condition--;
            ret = rt_condvar_notify_all(&static_condvar);

            if (ret != RT_EOK)
            {
                uassert_false(1);
            }
        } else if (condition == 1) {
            condition--;
            ret = rt_condvar_notify(&static_condvar);
            if (ret != RT_EOK)
            {
                uassert_false(1);
            }
        } else {
            static_condvar_notify_thread_finish = 1;
            ret = rt_mutex_release(&static_mutex);
            break;
        }
        ret = rt_mutex_release(&static_mutex);
        rt_thread_mdelay(10);
    }
}


static void test_static_condvar_wait_notify(void)
{
    rt_err_t result = RT_EOK;


    result  = rt_condvar_init(&static_condvar, "condvar", RT_IPC_FLAG_PRIO);
    if (result  != RT_EOK)
    {
        uassert_false(1);
    }

    rt_thread_init(&thread1,
                   "thread1",
                   thread1_condvar_wait,
                   RT_NULL,
                   &thread1_stack[0],
                   sizeof(thread1_stack),
                   THREAD_PRIORITY - 1, THREAD_TIMESLICE);
    rt_thread_startup(&thread1);

    rt_thread_init(&thread2,
                   "thread2",
                   thread2_notify_condvar,
                   RT_NULL,
                   &thread2_stack[0],
                   sizeof(thread2_stack),
                   THREAD_PRIORITY, THREAD_TIMESLICE);
    rt_thread_startup(&thread2);

    while(1)
    {
        if(static_condvar_wait_thread_finish && static_condvar_notify_thread_finish)
            return;
        else
            rt_thread_mdelay(10);
    }
}

static rt_err_t utest_tc_init(void)
{
    rt_err_t result = rt_mutex_init(&static_mutex, "mutex", RT_IPC_FLAG_PRIO);
    if (result  != RT_EOK)
    {
        uassert_false(1);
    }
    static_condvar_wait_thread_finish = 0;
    static_condvar_notify_thread_finish = 0;
    return result;
}

static rt_err_t utest_tc_cleanup(void)
{
    rt_condvar_detach(&static_condvar);
    rt_mutex_detach(&static_mutex);
    return RT_EOK;
}

static void testcase(void)
{
    UTEST_UNIT_RUN(test_condvar_init);
    UTEST_UNIT_RUN(test_condvar_detach);
    UTEST_UNIT_RUN(test_static_condvar_wait_notify);
}
UTEST_TC_EXPORT(testcase, "src.ipc.condvar_tc", utest_tc_init, utest_tc_cleanup, 60);
