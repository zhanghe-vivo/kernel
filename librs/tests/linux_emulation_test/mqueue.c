#include <mqueue.h>
#include <unistd.h>
#include <sys/stat.h>
#include <fcntl.h>

static const char *msg = "Hello, World!";
static const char *name = "/test_mq";
int main()
{
    mqd_t mq;

    mq = mq_open(name, O_CREAT | O_RDWR, S_IWUSR | S_IRUSR, NULL);
    if (mq == (mqd_t) -1)
    {
        write(1, "test failed\n", 12);
        return -1;
    }
    if (mq_send(mq, msg, 13, 1) != 0)
    {
        write(1, "test failed\n", 12);
        mq_close(mq);
        return -1;
    }
    mq_close(mq);
    mq_unlink(name);
    write(1, "test passed\n", 12);
    return 0;
}