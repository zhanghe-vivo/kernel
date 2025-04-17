#ifndef _LIBRS_WCHAR_H
#define _LIBRS_WCHAR_H

#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <time.h>
#include <bits/wchar.h>
#include <features.h>

typedef struct {

} mbstate_t;

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

wint_t btowc(int c);

wint_t fgetwc(FILE *stream);

wchar_t *fgetws(wchar_t *ws, int n, FILE *stream);

wint_t fputwc(wchar_t wc, FILE *stream);

int fputws(const wchar_t *ws, FILE *stream);

int fwide(FILE *stream, int mode);

wint_t getwc(FILE *stream);

wint_t getwchar(void);

int mbsinit(const mbstate_t *ps);

size_t mbrlen(const char *s, size_t n, mbstate_t *ps);

size_t mbrtowc(wchar_t *pwc, const char *s, size_t n, mbstate_t *ps);

size_t mbsnrtowcs(wchar_t *dst_ptr,
                  const char **src_ptr,
                  size_t src_len,
                  size_t dst_len,
                  mbstate_t *ps);

size_t mbsrtowcs(wchar_t *dst, const char **src, size_t len, mbstate_t *ps);

wint_t putwc(wchar_t wc, FILE *stream);

wint_t putwchar(wchar_t wc);

int vswscanf(const wchar_t *s, const wchar_t *format, va_list __valist);

int swscanf(const wchar_t *s, const wchar_t *format, ...);

/**
 * Push wide character `wc` back onto `stream` so it'll be read next
 */
wint_t ungetwc(wint_t wc, FILE *stream);

int vfwprintf(FILE *stream, const wchar_t *format, va_list arg);

int fwprintf(FILE *stream, const wchar_t *format, ...);

int vwprintf(const wchar_t *format, va_list arg);

int wprintf(const wchar_t *format, ...);

int vswprintf(wchar_t *s, size_t n, const wchar_t *format, va_list arg);

int swprintf(wchar_t *s, size_t n, const wchar_t *format, ...);

wchar_t *wcpcpy(wchar_t *d, const wchar_t *s);

wchar_t *wcpncpy(wchar_t *d, const wchar_t *s, size_t n);

size_t wcrtomb(char *s, wchar_t wc, mbstate_t *ps);

wchar_t *wcsdup(const wchar_t *s);

size_t wcsrtombs(char *s, const wchar_t **ws, size_t n, mbstate_t *st);

wchar_t *wcscat(wchar_t *ws1, const wchar_t *ws2);

/**
 * See <https://pubs.opengroup.org/onlinepubs/7908799/xsh/wcschr.html>.
 *
 * # Safety
 * The caller is required to ensure that `ws` is a valid pointer to a buffer
 * containing at least one nul value. The pointed-to buffer must not be
 * modified for the duration of the call.
 */
wchar_t *wcschr(const wchar_t *ws, wchar_t wc);

int wcscmp(const wchar_t *ws1, const wchar_t *ws2);

int wcscoll(const wchar_t *ws1, const wchar_t *ws2);

wchar_t *wcscpy(wchar_t *ws1, const wchar_t *ws2);

size_t wcscspn(const wchar_t *wcs, const wchar_t *set);

size_t wcslen(const wchar_t *ws);

wchar_t *wcsncat(wchar_t *ws1, const wchar_t *ws2, size_t n);

int wcsncmp(const wchar_t *ws1, const wchar_t *ws2, size_t n);

wchar_t *wcsncpy(wchar_t *ws1, const wchar_t *ws2, size_t n);

size_t wcsnlen(const wchar_t *s, size_t maxlen);

size_t wcsnrtombs(char *dest, const wchar_t **src, size_t nwc, size_t len, mbstate_t *ps);

wchar_t *wcspbrk(const wchar_t *wcs, const wchar_t *set);

wchar_t *wcsrchr(const wchar_t *ws1, wchar_t wc);

size_t wcsspn(const wchar_t *wcs, const wchar_t *set);

wchar_t *wcsstr(const wchar_t *ws1, const wchar_t *ws2);

double wcstod(const wchar_t *ptr, wchar_t **end);

wchar_t *wcstok(wchar_t *wcs, const wchar_t *delim, wchar_t **state);

long wcstol(const wchar_t *ptr, wchar_t **end, int base);

long long wcstoll(const wchar_t *ptr, wchar_t **end, int base);

intmax_t wcstoimax(const wchar_t *ptr, wchar_t **end, int base);

unsigned long wcstoul(const wchar_t *ptr, wchar_t **end, int base);

unsigned long long wcstoull(const wchar_t *ptr, wchar_t **end, int base);

uintmax_t wcstoumax(const wchar_t *ptr, wchar_t **end, int base);

wchar_t *wcswcs(const wchar_t *ws1, const wchar_t *ws2);

int wcswidth(const wchar_t *pwcs, size_t n);

int wctob(wint_t c);

int wcwidth(wchar_t wc);

wchar_t *wmemchr(const wchar_t *ws, wchar_t wc, size_t n);

int wmemcmp(const wchar_t *ws1, const wchar_t *ws2, size_t n);

wchar_t *wmemcpy(wchar_t *ws1, const wchar_t *ws2, size_t n);

wchar_t *wmemmove(wchar_t *ws1, const wchar_t *ws2, size_t n);

wchar_t *wmemset(wchar_t *ws, wchar_t wc, size_t n);

int vwscanf(const wchar_t *format, va_list __valist);

int wscanf(const wchar_t *format, ...);

int wcscasecmp(const wchar_t *s1, const wchar_t *s2);

int wcsncasecmp(const wchar_t *s1, const wchar_t *s2, size_t n);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* _LIBRS_WCHAR_H */
