#include <rtthread.h>
#include <msh.h>
#include <finsh.h>

int main(void)
{
    rt_kprintf("Hello Blue OS!\n");    rt_size_t total = 0, used = 0, max_used = 0, used_after_test = 0;

    rt_memory_info(&total, &used, &max_used);

    msh_exec("utest_run", 11);

    rt_memory_info(&total, &used_after_test, &max_used);

    if (used_after_test != used)
    {
        // rt_kprintf("used_before_test   : %d\n", used);
        // rt_kprintf("used_after_test    : %d\n", used_after_test);
        // rt_kprintf("memory leak\n");
    }

    while (1)
    {
        rt_thread_mdelay(5000);
    }
}