#ifndef _SYS_TIME_H
#define _SYS_TIME_H

#include <sys/types.h>
#include <features.h>

/**
 * See <https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/sys_time.h.html>.
 *
 * # Deprecation
 * The `ITIMER_REAL` symbolic constant was marked obsolescent in the Open
 * Group Base Specifications Issue 7, and removed in Issue 8.
 */
#define ITIMER_REAL 0

/**
 * See <https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/sys_time.h.html>.
 *
 * # Deprecation
 * The `ITIMER_VIRTUAL` symbolic constant was marked obsolescent in the Open
 * Group Base Specifications Issue 7, and removed in Issue 8.
 */
#define ITIMER_VIRTUAL 1

/**
 * See <https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/sys_time.h.html>.
 *
 * # Deprecation
 * The `ITIMER_PROF` symbolic constant was marked obsolescent in the Open
 * Group Base Specifications Issue 7, and removed in Issue 8.
 */
#define ITIMER_PROF 2

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/sys_time.h.html>.
 *
 * TODO: specified for `sys/select.h` in modern POSIX?
 */
struct timeval {
  time_t tv_sec;
  suseconds_t tv_usec;
};

/**
 * See <https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/sys_time.h.html>.
 *
 * # Deprecation
 * The `itimerval` struct was marked obsolescent in the Open Group Base
 * Specifications Issue 7, and removed in Issue 8.
 */
struct itimerval {
  struct timeval it_interval;
  struct timeval it_value;
};

/**
 * Non-POSIX, see <https://www.man7.org/linux/man-pages/man2/gettimeofday.2.html>.
 */
struct timezone {
  int tz_minuteswest;
  int tz_dsttime;
};

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * See <https://pubs.opengroup.org/onlinepubs/9699919799/functions/getitimer.html>.
 *
 * # Deprecation
 * The `getitimer()` function was marked obsolescent in the Open Group Base
 * Specifications Issue 7, and removed in Issue 8.
 */
__deprecated int getitimer(int which, struct itimerval *value);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9699919799/functions/gettimeofday.html>.
 *
 * See also <https://www.man7.org/linux/man-pages/man2/gettimeofday.2.html>
 * for further details on the `tzp` argument.
 *
 * # Deprecation
 * The `gettimeofday()` function was marked obsolescent in the Open Group Base
 * Specifications Issue 7, and removed in Issue 8.
 */
__deprecated int gettimeofday(struct timeval *tp, struct timezone *tzp);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9699919799/functions/getitimer.html>.
 *
 * # Deprecation
 * The `setitimer()` function was marked obsolescent in the Open Group Base
 * Specifications Issue 7, and removed in Issue 8.
 */
__deprecated int setitimer(int which, const struct itimerval *value, struct itimerval *ovalue);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/utimes.html>.
 *
 * # Deprecation
 * The `utimes()` function was marked legacy in the Open Group Base
 * Specifications Issue 6, and then unmarked in Issue 7.
 */
int utimes(const char *path, const struct timeval *times);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* _SYS_TIME_H */

#include <bits/sys/time.h>
