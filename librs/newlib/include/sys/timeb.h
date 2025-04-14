#ifndef _SYS_TIMEB_H
#define _SYS_TIMEB_H

#include <sys/types.h>

/**
 * See <https://pubs.opengroup.org/onlinepubs/009695399/basedefs/sys/timeb.h.html>.
 */
struct timeb {
  time_t time;
  unsigned short millitm;
  short timezone;
  short dstflag;
};

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * See <https://pubs.opengroup.org/onlinepubs/009695399/functions/ftime.html>.
 *
 * # Safety
 * The caller must ensure that `tp` is convertible to a `&mut
 * MaybeUninit<timeb>`.
 */
int ftime(struct timeb *tp);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* _SYS_TIMEB_H */
