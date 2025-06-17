#ifndef _LIBRS_TIME_H
#define _LIBRS_TIME_H

#include <sys/types.h>
#include <stdint.h>
#include <stddef.h>
#include <features.h>

#define CLOCKS_PER_SEC 1000000

#if defined(__linux__)
#define CLOCK_REALTIME 0
#endif

#if defined(__blueos__)
#define CLOCK_REALTIME 1
#endif

#if defined(__linux__)
#define CLOCK_MONOTONIC 1
#endif

#if defined(__blueos__)
#define CLOCK_MONOTONIC 4
#endif

#if defined(__linux__)
#define CLOCK_THREAD_CPUTIME_ID 3
#endif

#if defined(__linux__)
#define CLOCK_MONOTONIC_RAW 4
#endif

#if defined(__linux__)
#define CLOCK_REALTIME_COARSE 5
#endif

#if defined(__linux__)
#define CLOCK_MONOTONIC_COARSE 6
#endif

#if defined(__linux__)
#define CLOCK_BOOTTIME 7
#endif

#if defined(__linux__)
#define CLOCK_REALTIME_ALARM 8
#endif

#if defined(__linux__)
#define CLOCK_BOOTTIME_ALARM 9
#endif

#if defined(__linux__)
#define CLOCK_TAI 11
#endif

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/time.h.html>.
 */
struct tm {
  int tm_sec;
  int tm_min;
  int tm_hour;
  int tm_mday;
  int tm_mon;
  int tm_year;
  int tm_wday;
  int tm_yday;
  int tm_isdst;
  long tm_gmtoff;
  const char *tm_zone;
};

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/time.h.html>.
 */
struct timespec {
  time_t tv_sec;
  long tv_nsec;
};

typedef char *TzName[2];

#define CLOCK_PROCESS_CPUTIME_ID 2

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/time.h.html>.
 */
extern int daylight;

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/time.h.html>.
 */
extern long timezone;

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/time.h.html>.
 */
extern TzName tzname;

extern int getdate_err;

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/asctime.html>.
 *
 * # Deprecation
 * The `asctime()` function was marked obsolescent in the Open Group Base
 * Specifications Issue 7.
 */
__deprecated char *asctime(const struct tm *timeptr);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9699919799/functions/asctime.html>.
 *
 * # Deprecation
 * The `asctime_r()` was marked obsolescent in the Open Group Base
 * Specifications Issue 7, and removed in Issue 8.
 */
__deprecated char *asctime_r(const struct tm *tm, char *buf);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/clock.html>.
 */
clock_t clock(void);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/clock_getres.html>.
 */
int clock_getres(clockid_t clock_id, struct timespec *tp);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/clock_getres.html>.
 */
int clock_gettime(clockid_t clock_id, struct timespec *tp);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/clock_getres.html>.
 */
int clock_settime(clockid_t clock_id, const struct timespec *tp);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/ctime.html>.
 *
 * # Deprecation
 * The `ctime()` function was marked obsolescent in the Open Group Base
 * Specifications Issue 7.
 */
__deprecated char *ctime(const time_t *clock);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9699919799/functions/ctime.html>.
 *
 * # Deprecation
 * The `ctime_r()` function was marked obsolescent in the Open Group Base
 * Specifications Issue 7, and removed in Issue 8.
 */
__deprecated char *ctime_r(const time_t *clock, char *buf);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/difftime.html>.
 */
double difftime(time_t time1, time_t time0);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/gmtime.html>.
 */
struct tm *gmtime(const time_t *timer);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/gmtime.html>.
 */
struct tm *gmtime_r(const time_t *clock, struct tm *result);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/localtime.html>.
 */
struct tm *localtime(const time_t *clock);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/localtime.html>.
 */
struct tm *localtime_r(const time_t *clock, struct tm *t);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/mktime.html>.
 */
time_t mktime(struct tm *timeptr);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/nanosleep.html>.
 */
int nanosleep(const struct timespec *rqtp, struct timespec *rmtp);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/strftime.html>.
 */
size_t strftime(char *s, size_t maxsize, const char *format, const struct tm *timeptr);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/time.html>.
 */
time_t time(time_t *tloc);

/**
 * Non-POSIX, see <https://www.man7.org/linux/man-pages/man3/timegm.3.html>.
 */
time_t timegm(struct tm *tm);

/**
 * Non-POSIX, see <https://www.man7.org/linux/man-pages/man3/timegm.3.html>.
 */
__deprecated time_t timelocal(struct tm *tm);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/tzset.html>.
 */
void tzset(void);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/strptime.html>.
 */
char *strptime(const char *buf, const char *format, struct tm *tm);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* _LIBRS_TIME_H */
