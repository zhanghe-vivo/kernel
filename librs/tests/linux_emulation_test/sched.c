#include <unistd.h>
#include <sched.h>

int main()
{
    int priority_max = sched_get_priority_max(SCHED_FIFO);
    if (priority_max == -1)
    {
        write(1, "test failed\n", 12);
        return -1;
    }
    int priority_min = sched_get_priority_min(SCHED_FIFO);
    if (priority_min == -1)
    {
        write(1, "test failed\n", 12);
        return -1;
    }
    if (priority_max < priority_min)
    {
        write(1, "test failed\n", 12);
        return -1;
    }
    write(1, "test passed\n", 12);
    return 0;
}