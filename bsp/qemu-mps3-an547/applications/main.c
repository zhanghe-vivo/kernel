#include <rtthread.h>
#include <msh.h>
#include <finsh.h>

int main(void)
{
    rt_kprintf("Hello Blue OS!\n");
    msh_exec("utest_run",11);

    while (1)
    {
        rt_thread_mdelay(5000);
    }
}