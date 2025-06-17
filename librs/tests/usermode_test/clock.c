// This a simple example to test the librs clock_gettime() function in qemu-arm.
#include <time.h>
#include <unistd.h>
int main(int argc, char *argv[])
{
	struct timespec ts;

	if (clock_gettime(CLOCK_REALTIME, &ts) != 0) {
		write(1, "Test Failed\n", 12);
		return -1;
	}
	write(1, "Test passed\n", 12);
	return 0;
}