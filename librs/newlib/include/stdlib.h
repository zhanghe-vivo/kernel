#ifndef _LIBRS_STDLIB_H
#define _LIBRS_STDLIB_H

#include <stddef.h>
#include <alloca.h>
#include <wchar.h>
#include <features.h>

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/stdlib.h.html>.
 */
#define EXIT_FAILURE 1

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/stdlib.h.html>.
 */
#define EXIT_SUCCESS 0

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/stdlib.h.html>.
 */
#define RAND_MAX 2147483647

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/stdlib.h.html>.
 */
#define MB_CUR_MAX 4

/**
 * Actually specified for `limits.h`?
 */
#define MB_LEN_MAX 4

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/div.html>.
 */
typedef struct {
  int quot;
  int rem;
} div_t;

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/ldiv.html>.
 */
typedef struct {
  long quot;
  long rem;
} ldiv_t;

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/ldiv.html>.
 */
typedef struct {
  long long quot;
  long long rem;
} lldiv_t;

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

#if !defined(__LP64__)
extern const uintptr_t __stack_chk_guard;
#endif

#if defined(__LP64__)
extern const uintptr_t __stack_chk_guard;
#endif

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/_Exit.html>.
 */
void _Exit(int status) __noreturn;

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/a64l.html>.
 */
long a64l(const char *s);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/abort.html>.
 */
void abort(void) __noreturn;

void __stack_chk_fail(void) __noreturn;

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/abs.html>.
 */
int abs(int i);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/aligned_alloc.html>.
 */
void *aligned_alloc(size_t alignment, size_t size);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/at_quick_exit.html>.
 */
int at_quick_exit(void (*func)(void));

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/atexit.html>.
 */
int atexit(void (*func)(void));

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/atof.html>.
 */
double atof(const char *s);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/atoi.html>.
 */
int atoi(const char *s);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/atol.html>.
 */
long atol(const char *s);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/atol.html>.
 */
long long atoll(const char *s);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/bsearch.html>.
 */
void *bsearch(const void *key,
              const void *base,
              size_t nel,
              size_t width,
              int (*compar)(const void*, const void*));

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/calloc.html>.
 */
void *calloc(size_t nelem, size_t elsize);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/div.html>.
 */
div_t div(int numer, int denom);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/drand48.html>.
 *
 * # Panics
 * Panics if the function is unable to obtain a lock on the generator's global
 * state.
 */
double drand48(void);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/drand48.html>.
 *
 * # Safety
 * The caller must ensure that `xsubi` is convertible to a
 * `&mut [c_ushort; 3]`.
 *
 * # Panics
 * Panics if the function is unable to obtain a lock on the generator's global
 * state.
 */
double erand48(unsigned short *xsubi);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/exit.html>.
 */
void exit(int status) __noreturn;

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/free.html>.
 */
void free(void *ptr);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/getenv.html>.
 */
char *getenv(const char *name);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/getsubopt.html>.
 */
int getsubopt(char **optionp, char *const *tokens, char **valuep);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/grantpt.html>.
 */
int grantpt(int fildes);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/initstate.html>.
 */
char *initstate(unsigned int seed, char *state, size_t size);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/drand48.html>.
 *
 * # Safety
 * The caller must ensure that `xsubi` is convertible to a
 * `&mut [c_ushort; 3]`.
 *
 * # Panics
 * Panics if the function is unable to obtain a lock on the generator's global
 * state.
 */
long jrand48(unsigned short *xsubi);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/a64l.html>.
 */
char *l64a(long value);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/labs.html>.
 */
long labs(long i);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/drand48.html>.
 *
 * # Safety
 * The caller must ensure that `param` is convertible to a
 * `&mut [c_ushort; 7]`.
 *
 * # Panics
 * Panics if the function is unable to obtain a lock on the generator's global
 * state.
 */
void lcong48(unsigned short *param);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/ldiv.html>.
 */
ldiv_t ldiv(long numer, long denom);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/labs.html>.
 */
long long llabs(long long i);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/ldiv.html>.
 */
lldiv_t lldiv(long long numer, long long denom);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/drand48.html>.
 *
 * # Panics
 * Panics if the function is unable to obtain a lock on the generator's global
 * state.
 */
long lrand48(void);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/malloc.html>.
 */
void *malloc(size_t size);

/**
 * Non-POSIX, see <https://www.man7.org/linux/man-pages/man3/posix_memalign.3.html>.
 */
__deprecated void *memalign(size_t alignment, size_t size);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/mblen.html>.
 */
int mblen(const char *s, size_t n);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/mbstowcs.html>.
 */
size_t mbstowcs(wchar_t *pwcs, const char *s, size_t n);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/mbtowc.html>.
 */
int mbtowc(wchar_t *pwc, const char *s, size_t n);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/mkdtemp.html>.
 */
char *mkdtemp(char *name);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/mkdtemp.html>.
 */
int mkostemp(char *name, int flags);

/**
 * Non-POSIX, see <https://www.man7.org/linux/man-pages/man3/mkstemp.3.html>.
 */
int mkostemps(char *name, int suffix_len, int flags);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/mkdtemp.html>.
 */
int mkstemp(char *name);

/**
 * Non-POSIX, see <https://www.man7.org/linux/man-pages/man3/mkstemp.3.html>.
 */
int mkstemps(char *name, int suffix_len);

/**
 * See <https://pubs.opengroup.org/onlinepubs/009695399/functions/mktemp.html>.
 *
 * # Deprecation
 * The `mktemp()` function was marked as legacy in the Open Group Base
 * Specifications Issue 6, and the function was removed in Issue 7.
 */
__deprecated char *mktemp(char *name);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/drand48.html>.
 *
 * # Panics
 * Panics if the function is unable to obtain a lock on the generator's global
 * state.
 */
long mrand48(void);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/drand48.html>.
 *
 * # Safety
 * The caller must ensure that `xsubi` is convertible to a
 * `&mut [c_ushort; 3]`.
 *
 * # Panics
 * Panics if the function is unable to obtain a lock on the generator's global
 * state.
 */
long nrand48(unsigned short *xsubi);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/posix_memalign.html>.
 */
int posix_memalign(void **memptr, size_t alignment, size_t size);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/posix_openpt.html>.
 */
int posix_openpt(int flags);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/ptsname.html>.
 */
char *ptsname(int fd);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/ptsname.html>.
 */
int ptsname_r(int fd, char *buf, size_t buflen);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/putenv.html>.
 */
int putenv(char *insert);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/qsort.html>.
 */
void qsort(void *base, size_t nel, size_t width, int (*compar)(const void*, const void*));

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/rand.html>.
 */
int rand(void);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9699919799/functions/rand.html>.
 *
 * # Deprecation
 * The `rand_r()` function was marked as obsolescent in the Open Group Base
 * Specifications Issue 7, and the function was removed in Issue 8.
 */
int rand_r(unsigned int *seed);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/initstate.html>.
 */
long random(void);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/realloc.html>.
 */
void *realloc(void *ptr, size_t size);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/realloc.html>.
 */
void *reallocarray(void *ptr, size_t m, size_t n);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/realpath.html>.
 */
char *realpath(const char *pathname, char *resolved);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/drand48.html>.
 *
 * # Safety
 * The caller must ensure that `seed16v` is convertible to a `&[c_ushort; 3]`.
 * Additionally, the caller must ensure that the function has exclusive access
 * to the static buffer it returns; this includes avoiding simultaneous calls
 * to this function.
 *
 * # Panics
 * Panics if the function is unable to obtain a lock on the generator's global
 * state.
 */
unsigned short *seed48(unsigned short *seed16v);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/setenv.html>.
 */
int setenv(const char *key, const char *value, int overwrite);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/initstate.html>.
 */
char *setstate(char *state);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/rand.html>.
 */
void srand(unsigned int seed);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/drand48.html>.
 *
 * # Panics
 * Panics if the function is unable to obtain a lock on the generator's global
 * state.
 */
void srand48(long seedval);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/initstate.html>.
 */
void srandom(unsigned int seed);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/strtod.html>.
 */
double strtod(const char *s, char **endptr);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/strtod.html>.
 */
float strtof(const char *s, char **endptr);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/strtol.html>.
 */
long strtol(const char *s, char **endptr, int base);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/strtol.html>.
 */
long long strtoll(const char *s, char **endptr, int base);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/strtoul.html>.
 */
unsigned long strtoul(const char *s, char **endptr, int base);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/strtoul.html>.
 */
unsigned long long strtoull(const char *s, char **endptr, int base);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/system.html>.
 */
int system(const char *command);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/unlockpt.html>.
 */
int unlockpt(int fildes);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/unsetenv.html>.
 */
int unsetenv(const char *key);

/**
 * See <https://pubs.opengroup.org/onlinepubs/7908799/xsh/valloc.html>.
 *
 * # Deprecation
 * The `valloc()` function was marked as obsolescent in the Open Group Base
 * Specifications Issue 5, and the function was removed in Issue 6.
 */
__deprecated void *valloc(size_t size);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/wcstombs.html>.
 */
size_t wcstombs(char *s, const wchar_t *pwcs, size_t n);

/**
 * See <https://pubs.opengroup.org/onlinepubs/9799919799/functions/wctomb.html>.
 */
int wctomb(char *s, wchar_t wc);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* _LIBRS_STDLIB_H */

#include <bits/stdlib.h>
