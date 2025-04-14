#ifndef _SYS_TIMES_H
#define _SYS_TIMES_H

#include <sys/types.h>

struct tms {
  clock_t tms_utime;
  clock_t tms_stime;
  clock_t tms_cutime;
  clock_t tms_cstime;
};

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

clock_t times(struct tms *out);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* _SYS_TIMES_H */
