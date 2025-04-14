#ifndef _LIBRS_INTTYPES_H
#define _LIBRS_INTTYPES_H

#include <stdint.h>
#include <wchar.h>

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/inttypes.h.html>.
 */
typedef struct {
  intmax_t quot;
  intmax_t rem;
} imaxdiv_t;

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/imaxabs.html>.
 */
intmax_t imaxabs(intmax_t i);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/imaxdiv.html>.
 */
imaxdiv_t imaxdiv(intmax_t i, intmax_t j);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/strtoimax.html>.
 */
intmax_t strtoimax(const char *s, char **endptr, int base);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/strtoimax.html>.
 */
uintmax_t strtoumax(const char *s, char **endptr, int base);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* _LIBRS_INTTYPES_H */

#include <bits/inttypes.h>
