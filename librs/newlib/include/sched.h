#ifndef _LIBRS_SCHED_H
#define _LIBRS_SCHED_H

#include <time.h>
#include <bits/sched.h>

#define SCHED_FIFO 0

#define SCHED_RR 1

#define SCHED_OTHER 2

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

int sched_yield(void);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* _LIBRS_SCHED_H */
