#include <rtthread.h>
#include "utest.h"
#include <stdlib.h>
#include <stdbool.h>


static struct rt_rwlock static_rwlock = {0};

static rt_uint32_t static_readlock1_thread_finish = 0,
                    static_readlock2_thread_finish = 0,
                    static_writelock_thread_finish = 0;

rt_align(RT_ALIGN_SIZE)
static char thread1_stack[UTEST_THR_STACK_SIZE];
static struct rt_thread thread1;

rt_align(RT_ALIGN_SIZE)
static char thread2_stack[UTEST_THR_STACK_SIZE];
static struct rt_thread thread2;

rt_align(RT_ALIGN_SIZE)
static char thread3_stack[UTEST_THR_STACK_SIZE];
static struct rt_thread thread3;

#define THREAD_PRIORITY      9
#define THREAD_TIMESLICE     5

static void test_rwlock_init(void)
{
    rt_err_t result;


    result = rt_rwlock_init(&static_rwlock, "rwlock", RT_IPC_FLAG_PRIO);
    if (result != RT_EOK)
    {
        uassert_false(1);
    }
    result = rt_rwlock_detach(&static_rwlock);
    if (result != RT_EOK)
    {
        uassert_false(1);
    }

    result = rt_rwlock_init(&static_rwlock, "rwlock", RT_IPC_FLAG_FIFO);
    if (result != RT_EOK)
    {
        uassert_false(1);
    }
    result = rt_rwlock_detach(&static_rwlock);
    if (result != RT_EOK)
    {
        uassert_false(1);
    }

    uassert_true(1);
}

static void test_rwlock_detach(void)
{
    rt_err_t result = RT_EOK;

    result = rt_rwlock_init(&static_rwlock, "rwlock", RT_IPC_FLAG_PRIO);
    if (result != RT_EOK)
    {
        uassert_false(1);
    }

    result = rt_rwlock_detach(&static_rwlock);
    if (result != RT_EOK)
    {
        uassert_false(1);
    }

    uassert_true(1);
}

static void thread1_readlock(void *param)
{
    rt_err_t ret = RT_EOK;

    ret = rt_rwlock_lock_read(&static_rwlock);

    static_readlock1_thread_finish = 1;
}

static void thread2_readlock(void *param)
{
    rt_err_t ret = RT_EOK;

    ret = rt_rwlock_lock_read(&static_rwlock);

    static_readlock2_thread_finish = 1;
}

static void thread3_writelock(void *param)
{
    rt_err_t ret = RT_EOK;

    int rc = 2;
    while(1) {
        ret = rt_rwlock_try_lock_write(&static_rwlock);

        if (ret != RT_EOK)
        {
            if (rc >= 0) {
                ret = rt_rwlock_unlock(&static_rwlock);
                if(ret == RT_EOK)
                    rc--;
            }else{
                uassert_false(1);
            }
        } else {
            break;
        }
    }

    ret = rt_rwlock_unlock(&static_rwlock);
    if (ret != RT_EOK)
    {
        uassert_false(1);
    }

    static_writelock_thread_finish = 1;
}

static void test_static_rwlock(void)
{

    rt_err_t result = RT_EOK;


    result  = rt_rwlock_init(&static_rwlock, "rwlock", RT_IPC_FLAG_PRIO);
    if (result  != RT_EOK)
    {
        uassert_false(1);
    }

    rt_thread_init(&thread1,
                   "thread1",
                   thread1_readlock,
                   RT_NULL,
                   &thread1_stack[0],
                   sizeof(thread1_stack),
                   THREAD_PRIORITY - 1, THREAD_TIMESLICE);
    rt_thread_startup(&thread1);

    rt_thread_init(&thread2,
                   "thread2",
                   thread2_readlock,
                   RT_NULL,
                   &thread2_stack[0],
                   sizeof(thread2_stack),
                   THREAD_PRIORITY - 1, THREAD_TIMESLICE);
    rt_thread_startup(&thread2);

    rt_thread_init(&thread3,
                   "thread3",
                   thread3_writelock,
                   RT_NULL,
                   &thread3_stack[0],
                   sizeof(thread3_stack),
                   THREAD_PRIORITY, THREAD_TIMESLICE);
    rt_thread_startup(&thread3);

    while(1) {
        if(static_readlock1_thread_finish && static_readlock2_thread_finish && static_writelock_thread_finish) {
            uassert_true(1);
            return;
        }
        else
            rt_thread_mdelay(10);
    }


}

static rt_err_t utest_tc_init(void)
{
    return RT_EOK;
}

static rt_err_t utest_tc_cleanup(void)
{
    return RT_EOK;
}

static void testcase(void)
{
    UTEST_UNIT_RUN(test_rwlock_init);
    UTEST_UNIT_RUN(test_rwlock_detach);
    UTEST_UNIT_RUN(test_static_rwlock);
}
UTEST_TC_EXPORT(testcase, "src.ipc.rwlock_tc", utest_tc_init, utest_tc_cleanup, 60);
