/*
 * Copyright (c) 2006-2022, RT-Thread Development Team
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * Change Logs:
 * Date           Author       Notes
 * 2006-03-16     Bernard      the first version
 * 2006-05-25     Bernard      rewrite vsprintf
 * 2006-08-10     Bernard      add rt_show_version
 * 2010-03-17     Bernard      remove rt_strlcpy function
 *                             fix gcc compiling issue.
 * 2010-04-15     Bernard      remove weak definition on ICCM16C compiler
 * 2012-07-18     Arda         add the alignment display for signed integer
 * 2012-11-23     Bernard      fix IAR compiler error.
 * 2012-12-22     Bernard      fix rt_kprintf issue, which found by Grissiom.
 * 2013-06-24     Bernard      remove rt_kprintf if RT_USING_CONSOLE is not defined.
 * 2013-09-24     aozima       make sure the device is in STREAM mode when used by rt_kprintf.
 * 2015-07-06     Bernard      Add rt_assert_handler routine.
 * 2021-02-28     Meco Man     add RT_KSERVICE_USING_STDLIB
 * 2021-12-20     Meco Man     implement rt_strcpy()
 * 2022-01-07     Gabriel      add __on_rt_assert_hook
 * 2022-06-04     Meco Man     remove strnlen
 */

 #include <rtthread.h>
 #include <rthw.h>
 #include <stdlib.h>
 
 #ifdef RT_USING_MODULE
 #include <dlmodule.h>
 #endif /* RT_USING_MODULE */
 
//  #include "ipc/ringbuffer.h"
 
 /* use precision */
 #define RT_PRINTF_PRECISION
 
 /**
  * @addtogroup KernelService
  */
 
 /**@{*/
 
 /* global errno in RT-Thread */
 static volatile int __rt_errno;
 
 #if defined(RT_USING_DEVICE) && defined(RT_USING_CONSOLE)
 static rt_device_t _console_device = RT_NULL;
 static rt_device_t _console_default_device = RT_NULL;
 #endif
 
 RT_WEAK void rt_hw_us_delay(rt_uint32_t us)
 {
     (void) us;
     RT_DEBUG_LOG(RT_DEBUG_DEVICE, ("rt_hw_us_delay() doesn't support for this board."
         "Please consider implementing rt_hw_us_delay() in another file.\n"));
 }
 
 static const char* rt_errno_strs[] =
 {
     "OK",
     "ERROR",
     "ETIMOUT",
     "ERSFULL",
     "ERSEPTY",
     "ENOMEM",
     "ENOSYS",
     "EBUSY",
     "EIO",
     "EINTRPT",
     "EINVAL",
     "EUNKNOW"
 };
 
 /**
  * This function return a pointer to a string that contains the
  * message of error.
  *
  * @param error the errorno code
  * @return a point to error message string
  */
 const char *rt_strerror(rt_err_t error)
 {
     if (error < 0)
         error = -error;
 
     return (error > RT_EINVAL + 1) ?
            rt_errno_strs[RT_EINVAL + 1] :
            rt_errno_strs[error];
 }
 RTM_EXPORT(rt_strerror);
 
 /**
  * This function gets the global errno for the current thread.
  *
  * @return errno
  */
 rt_err_t rt_get_errno(void)
 {
     rt_thread_t tid;
 
     if (rt_interrupt_get_nest() != 0)
     {
         /* it's in interrupt context */
         return __rt_errno;
     }
 
     tid = rt_thread_self();
     if (tid == RT_NULL)
         return __rt_errno;
 
     // return tid->error;
     return rt_get_thread_errno(tid);
 }
 RTM_EXPORT(rt_get_errno);
 
 /**
  * This function sets the global errno for the current thread.
  *
  * @param error is the errno shall be set.
  */
 void rt_set_errno(rt_err_t error)
 {
     rt_thread_t tid;
 
     if (rt_interrupt_get_nest() != 0)
     {
         /* it's in interrupt context */
         __rt_errno = error;
 
         return;
     }
 
     tid = rt_thread_self();
     if (tid == RT_NULL)
     {
         __rt_errno = error;
 
         return;
     }

     // tid->error = error;
     rt_set_thread_errno(tid, error);
 }
 RTM_EXPORT(rt_set_errno);
 
 /**
  * This function returns the address of the current thread errno.
  *
  * @return The errno address.
  */
 int *_rt_errno(void)
 {
     rt_thread_t tid;
 
     if (rt_interrupt_get_nest() != 0)
         return (int *)&__rt_errno;
 
     tid = rt_thread_self();
     if (tid != RT_NULL)
         //return (int *) & (tid->error);
         return (int *) rt_get_thread_errno_addr(tid);
 
     return (int *)&__rt_errno;
 }
 RTM_EXPORT(_rt_errno);
 
 #ifndef RT_KSERVICE_USING_STDLIB_MEMORY
 /**
  * This function will set the content of memory to specified value.
  *
  * @param  s is the address of source memory, point to the memory block to be filled.
  *
  * @param  c is the value to be set. The value is passed in int form, but the function
  *         uses the unsigned character form of the value when filling the memory block.
  *
  * @param  count number of bytes to be set.
  *
  * @return The address of source memory.
  */
 RT_WEAK void *rt_memset(void *s, int c, rt_ubase_t count)
 {
 #ifdef RT_KSERVICE_USING_TINY_SIZE
     char *xs = (char *)s;
 
     while (count--)
         *xs++ = c;
 
     return s;
 #else
 #define LBLOCKSIZE      (sizeof(long))
 #define UNALIGNED(X)    ((long)X & (LBLOCKSIZE - 1))
 #define TOO_SMALL(LEN)  ((LEN) < LBLOCKSIZE)
 
     unsigned int i;
     char *m = (char *)s;
     unsigned long buffer;
     unsigned long *aligned_addr;
     unsigned int d = c & 0xff;  /* To avoid sign extension, copy C to an
                                 unsigned variable.  */
 
     if (!TOO_SMALL(count) && !UNALIGNED(s))
     {
         /* If we get this far, we know that count is large and s is word-aligned. */
         aligned_addr = (unsigned long *)s;
 
         /* Store d into each char sized location in buffer so that
          * we can set large blocks quickly.
          */
         if (LBLOCKSIZE == 4)
         {
             buffer = (d << 8) | d;
             buffer |= (buffer << 16);
         }
         else
         {
             buffer = 0;
             for (i = 0; i < LBLOCKSIZE; i ++)
                 buffer = (buffer << 8) | d;
         }
 
         while (count >= LBLOCKSIZE * 4)
         {
             *aligned_addr++ = buffer;
             *aligned_addr++ = buffer;
             *aligned_addr++ = buffer;
             *aligned_addr++ = buffer;
             count -= 4 * LBLOCKSIZE;
         }
 
         while (count >= LBLOCKSIZE)
         {
             *aligned_addr++ = buffer;
             count -= LBLOCKSIZE;
         }
 
         /* Pick up the remainder with a bytewise loop. */
         m = (char *)aligned_addr;
     }
 
     while (count--)
     {
         *m++ = (char)d;
     }
 
     return s;
 
 #undef LBLOCKSIZE
 #undef UNALIGNED
 #undef TOO_SMALL
 #endif /* RT_KSERVICE_USING_TINY_SIZE */
 }
 RTM_EXPORT(rt_memset);
 
 /**
  * This function will copy memory content from source address to destination address.
  *
  * @param  dst is the address of destination memory, points to the copied content.
  *
  * @param  src  is the address of source memory, pointing to the data source to be copied.
  *
  * @param  count is the copied length.
  *
  * @return The address of destination memory
  */
 RT_WEAK void *rt_memcpy(void *dst, const void *src, rt_ubase_t count)
 {
 #ifdef RT_KSERVICE_USING_TINY_SIZE
     char *tmp = (char *)dst, *s = (char *)src;
     rt_ubase_t len;
 
     if (tmp <= s || tmp > (s + count))
     {
         while (count--)
             *tmp ++ = *s ++;
     }
     else
     {
         for (len = count; len > 0; len --)
             tmp[len - 1] = s[len - 1];
     }
 
     return dst;
 #else
 
 #define UNALIGNED(X, Y) \
     (((long)X & (sizeof (long) - 1)) | ((long)Y & (sizeof (long) - 1)))
 #define BIGBLOCKSIZE    (sizeof (long) << 2)
 #define LITTLEBLOCKSIZE (sizeof (long))
 #define TOO_SMALL(LEN)  ((LEN) < BIGBLOCKSIZE)
 
     char *dst_ptr = (char *)dst;
     char *src_ptr = (char *)src;
     long *aligned_dst;
     long *aligned_src;
     rt_ubase_t len = count;
 
     /* If the size is small, or either SRC or DST is unaligned,
     then punt into the byte copy loop.  This should be rare. */
     if (!TOO_SMALL(len) && !UNALIGNED(src_ptr, dst_ptr))
     {
         aligned_dst = (long *)dst_ptr;
         aligned_src = (long *)src_ptr;
 
         /* Copy 4X long words at a time if possible. */
         while (len >= BIGBLOCKSIZE)
         {
             *aligned_dst++ = *aligned_src++;
             *aligned_dst++ = *aligned_src++;
             *aligned_dst++ = *aligned_src++;
             *aligned_dst++ = *aligned_src++;
             len -= BIGBLOCKSIZE;
         }
 
         /* Copy one long word at a time if possible. */
         while (len >= LITTLEBLOCKSIZE)
         {
             *aligned_dst++ = *aligned_src++;
             len -= LITTLEBLOCKSIZE;
         }
 
         /* Pick up any residual with a byte copier. */
         dst_ptr = (char *)aligned_dst;
         src_ptr = (char *)aligned_src;
     }
 
     while (len--)
         *dst_ptr++ = *src_ptr++;
 
     return dst;
 #undef UNALIGNED
 #undef BIGBLOCKSIZE
 #undef LITTLEBLOCKSIZE
 #undef TOO_SMALL
 #endif /* RT_KSERVICE_USING_TINY_SIZE */
 }
 RTM_EXPORT(rt_memcpy);
 
 /**
  * This function will move memory content from source address to destination
  * address. If the destination memory does not overlap with the source memory,
  * the function is the same as memcpy().
  *
  * @param  dest is the address of destination memory, points to the copied content.
  *
  * @param  src is the address of source memory, point to the data source to be copied.
  *
  * @param  n is the copied length.
  *
  * @return The address of destination memory.
  */
 void *rt_memmove(void *dest, const void *src, rt_size_t n)
 {
     char *tmp = (char *)dest, *s = (char *)src;
 
     if (s < tmp && tmp < s + n)
     {
         tmp += n;
         s += n;
 
         while (n--)
             *(--tmp) = *(--s);
     }
     else
     {
         while (n--)
             *tmp++ = *s++;
     }
 
     return dest;
 }
 RTM_EXPORT(rt_memmove);
 
 /**
  * This function will compare two areas of memory.
  *
  * @param  cs is a block of memory.
  *
  * @param  ct is another block of memory.
  *
  * @param  count is the size of the area.
  *
  * @return Compare the results:
  *         If the result < 0, cs is smaller than ct.
  *         If the result > 0, cs is greater than ct.
  *         If the result = 0, cs is equal to ct.
  */
 rt_int32_t rt_memcmp(const void *cs, const void *ct, rt_size_t count)
 {
     const unsigned char *su1, *su2;
     int res = 0;
 
     for (su1 = (const unsigned char *)cs, su2 = (const unsigned char *)ct; 0 < count; ++su1, ++su2, count--)
         if ((res = *su1 - *su2) != 0)
             break;
 
     return res;
 }
 RTM_EXPORT(rt_memcmp);
 #endif /* RT_KSERVICE_USING_STDLIB_MEMORY*/
 
 #ifndef RT_KSERVICE_USING_STDLIB
 /**
  * This function will return the first occurrence of a string, without the
  * terminator '\0'.
  *
  * @param  s1 is the source string.
  *
  * @param  s2 is the find string.
  *
  * @return The first occurrence of a s2 in s1, or RT_NULL if no found.
  */
 char *rt_strstr(const char *s1, const char *s2)
 {
     int l1, l2;
 
     l2 = rt_strlen(s2);
     if (!l2)
         return (char *)s1;
     l1 = rt_strlen(s1);
     while (l1 >= l2)
     {
         l1 --;
         if (!rt_memcmp(s1, s2, l2))
             return (char *)s1;
         s1 ++;
     }
 
     return RT_NULL;
 }
 RTM_EXPORT(rt_strstr);
 
 /**
  * This function will compare two strings while ignoring differences in case
  *
  * @param  a is the string to be compared.
  *
  * @param  b is the string to be compared.
  *
  * @return Compare the results:
  *         If the result < 0, a is smaller than a.
  *         If the result > 0, a is greater than a.
  *         If the result = 0, a is equal to a.
  */
 rt_int32_t rt_strcasecmp(const char *a, const char *b)
 {
     int ca, cb;
 
     do
     {
         ca = *a++ & 0xff;
         cb = *b++ & 0xff;
         if (ca >= 'A' && ca <= 'Z')
             ca += 'a' - 'A';
         if (cb >= 'A' && cb <= 'Z')
             cb += 'a' - 'A';
     }
     while (ca == cb && ca != '\0');
 
     return ca - cb;
 }
 RTM_EXPORT(rt_strcasecmp);
 
 /**
  * This function will copy string no more than n bytes.
  *
  * @param  dst points to the address used to store the copied content.
  *
  * @param  src is the string to be copied.
  *
  * @param  n is the maximum copied length.
  *
  * @return The address where the copied content is stored.
  */
 char *rt_strncpy(char *dst, const char *src, rt_size_t n)
 {
     if (n != 0)
     {
         char *d = dst;
         const char *s = src;
 
         do
         {
             if ((*d++ = *s++) == 0)
             {
                 /* NUL pad the remaining n-1 bytes */
                 while (--n != 0)
                     *d++ = 0;
                 break;
             }
         } while (--n != 0);
     }
 
     return (dst);
 }
 RTM_EXPORT(rt_strncpy);
 
 /**
  * This function will copy string.
  *
  * @param  dst points to the address used to store the copied content.
  *
  * @param  src is the string to be copied.
  *
  * @return The address where the copied content is stored.
  */
 char *rt_strcpy(char *dst, const char *src)
 {
     char *dest = dst;
 
     while (*src != '\0')
     {
         *dst = *src;
         dst++;
         src++;
     }
 
     *dst = '\0';
     return dest;
 }
 RTM_EXPORT(rt_strcpy);
 
 /**
  * This function will compare two strings with specified maximum length.
  *
  * @param  cs is the string to be compared.
  *
  * @param  ct is the string to be compared.
  *
  * @param  count is the maximum compare length.
  *
  * @return Compare the results:
  *         If the result < 0, cs is smaller than ct.
  *         If the result > 0, cs is greater than ct.
  *         If the result = 0, cs is equal to ct.
  */
 rt_int32_t rt_strncmp(const char *cs, const char *ct, rt_size_t count)
 {
     signed char __res = 0;
 
     while (count)
     {
         if ((__res = *cs - *ct++) != 0 || !*cs++)
             break;
         count --;
     }
 
     return __res;
 }
 RTM_EXPORT(rt_strncmp);
 
 /**
  * This function will compare two strings without specified length.
  *
  * @param  cs is the string to be compared.
  *
  * @param  ct is the string to be compared.
  *
  * @return Compare the results:
  *         If the result < 0, cs is smaller than ct.
  *         If the result > 0, cs is greater than ct.
  *         If the result = 0, cs is equal to ct.
  */
 rt_int32_t rt_strcmp(const char *cs, const char *ct)
 {
     while (*cs && *cs == *ct)
     {
         cs++;
         ct++;
     }
 
     return (*cs - *ct);
 }
 RTM_EXPORT(rt_strcmp);
 
 /**
  * This function will return the length of a string, which terminate will
  * null character.
  *
  * @param  s is the string
  *
  * @return The length of string.
  */
 rt_size_t rt_strlen(const char *s)
 {
     const char *sc;
 
     for (sc = s; *sc != '\0'; ++sc) /* nothing */
         ;
 
     return sc - s;
 }
 RTM_EXPORT(rt_strlen);
 
 #endif /* RT_KSERVICE_USING_STDLIB */
 
 /**
  * The  strnlen()  function  returns the number of characters in the
  * string pointed to by s, excluding the terminating null byte ('\0'),
  * but at most maxlen.  In doing this, strnlen() looks only at the
  * first maxlen characters in the string pointed to by s and never
  * beyond s+maxlen.
  *
  * @param  s is the string.
  *
  * @param  maxlen is the max size.
  *
  * @return The length of string.
  */
 rt_size_t rt_strnlen(const char *s, rt_ubase_t maxlen)
 {
     const char *sc;
 
     for (sc = s; *sc != '\0' && (rt_ubase_t)(sc - s) < maxlen; ++sc) /* nothing */
         ;
 
     return sc - s;
 }
 RTM_EXPORT(rt_strnlen);
 
 #ifdef RT_USING_HEAP
 /**
  * This function will duplicate a string.
  *
  * @param  s is the string to be duplicated.
  *
  * @return The string address of the copy.
  */
 char *rt_strdup(const char *s)
 {
     rt_size_t len = rt_strlen(s) + 1;
     char *tmp = (char *)rt_malloc(len);
 
     if (!tmp)
         return RT_NULL;
 
     rt_memcpy(tmp, s, len);
 
     return tmp;
 }
 RTM_EXPORT(rt_strdup);
 #ifdef __ARMCC_VERSION
 char *strdup(const char *s) __attribute__((alias("rt_strdup")));
 #endif /* __ARMCC_VERSION */
 #endif /* RT_USING_HEAP */
 
 /**
  * This function will show the version of rt-thread rtos
  */
 void rt_show_version(void)
 {
     rt_kprintf("\n \\ | /\n");
     rt_kprintf("- RT -     Thread Operating System\n");
     rt_kprintf(" / | \\     %d.%d.%d build %s\n",
                RT_VERSION, RT_SUBVERSION, RT_REVISION, __DATE__);
     rt_kprintf(" 2006 - 2020 Copyright by rt-thread team\n");
 }
 RTM_EXPORT(rt_show_version);
 
 /* private function */
 #define _ISDIGIT(c)  ((unsigned)((c) - '0') < 10)
 
 /**
  * This function will duplicate a string.
  *
  * @param  n is the string to be duplicated.
  *
  * @param  base is support divide instructions value.
  *
  * @return the duplicated string pointer.
  */
 #ifdef RT_KPRINTF_USING_LONGLONG
 rt_inline int divide(long long *n, int base)
 #else
 rt_inline int divide(long *n, int base)
 #endif /* RT_KPRINTF_USING_LONGLONG */
 {
     int res;
 
     /* optimized for processor which does not support divide instructions. */
     if (base == 10)
     {
 #ifdef RT_KPRINTF_USING_LONGLONG
         res = (int)(((unsigned long long)*n) % 10U);
         *n = (long long)(((unsigned long long)*n) / 10U);
 #else
         res = (int)(((unsigned long)*n) % 10U);
         *n = (long)(((unsigned long)*n) / 10U);
 #endif
     }
     else
     {
 #ifdef RT_KPRINTF_USING_LONGLONG
         res = (int)(((unsigned long long)*n) % 16U);
         *n = (long long)(((unsigned long long)*n) / 16U);
 #else
         res = (int)(((unsigned long)*n) % 16U);
         *n = (long)(((unsigned long)*n) / 16U);
 #endif
     }
 
     return res;
 }
 
 rt_inline int skip_atoi(const char **s)
 {
     int i = 0;
     while (_ISDIGIT(**s))
         i = i * 10 + *((*s)++) - '0';
 
     return i;
 }
 
 #define ZEROPAD     (1 << 0)    /* pad with zero */
 #define SIGN        (1 << 1)    /* unsigned/signed long */
 #define PLUS        (1 << 2)    /* show plus */
 #define SPACE       (1 << 3)    /* space if plus */
 #define LEFT        (1 << 4)    /* left justified */
 #define SPECIAL     (1 << 5)    /* 0x */
 #define LARGE       (1 << 6)    /* use 'ABCDEF' instead of 'abcdef' */
 
 static char *print_number(char *buf,
                           char *end,
 #ifdef RT_KPRINTF_USING_LONGLONG
                           long long  num,
 #else
                           long  num,
 #endif /* RT_KPRINTF_USING_LONGLONG */
                           int   base,
                           int   s,
 #ifdef RT_PRINTF_PRECISION
                           int   precision,
 #endif /* RT_PRINTF_PRECISION */
                           int   type)
 {
     char c, sign;
 #ifdef RT_KPRINTF_USING_LONGLONG
     char tmp[32];
 #else
     char tmp[16];
 #endif /* RT_KPRINTF_USING_LONGLONG */
     int precision_bak = precision;
     const char *digits;
     static const char small_digits[] = "0123456789abcdef";
     static const char large_digits[] = "0123456789ABCDEF";
     int i, size;
 
     size = s;
 
     digits = (type & LARGE) ? large_digits : small_digits;
     if (type & LEFT)
         type &= ~ZEROPAD;
 
     c = (type & ZEROPAD) ? '0' : ' ';
 
     /* get sign */
     sign = 0;
     if (type & SIGN)
     {
         if (num < 0)
         {
             sign = '-';
             num = -num;
         }
         else if (type & PLUS)
             sign = '+';
         else if (type & SPACE)
             sign = ' ';
     }
 
 #ifdef RT_PRINTF_SPECIAL
     if (type & SPECIAL)
     {
         if (base == 16)
             size -= 2;
         else if (base == 8)
             size--;
     }
 #endif /* RT_PRINTF_SPECIAL */
 
     i = 0;
     if (num == 0)
         tmp[i++] = '0';
     else
     {
         while (num != 0)
             tmp[i++] = digits[divide(&num, base)];
     }
 
 #ifdef RT_PRINTF_PRECISION
     if (i > precision)
         precision = i;
     size -= precision;
 #else
     size -= i;
 #endif /* RT_PRINTF_PRECISION */
 
     if (!(type & (ZEROPAD | LEFT)))
     {
         if ((sign) && (size > 0))
             size--;
 
         while (size-- > 0)
         {
             if (buf < end)
                 *buf = ' ';
             ++ buf;
         }
     }
 
     if (sign)
     {
         if (buf < end)
         {
             *buf = sign;
         }
         -- size;
         ++ buf;
     }
 
 #ifdef RT_PRINTF_SPECIAL
     if (type & SPECIAL)
     {
         if (base == 8)
         {
             if (buf < end)
                 *buf = '0';
             ++ buf;
         }
         else if (base == 16)
         {
             if (buf < end)
                 *buf = '0';
             ++ buf;
             if (buf < end)
             {
                 *buf = type & LARGE ? 'X' : 'x';
             }
             ++ buf;
         }
     }
 #endif /* RT_PRINTF_SPECIAL */
 
     /* no align to the left */
     if (!(type & LEFT))
     {
         while (size-- > 0)
         {
             if (buf < end)
                 *buf = c;
             ++ buf;
         }
     }
 
 #ifdef RT_PRINTF_PRECISION
     while (i < precision--)
     {
         if (buf < end)
             *buf = '0';
         ++ buf;
     }
 #endif /* RT_PRINTF_PRECISION */
 
     /* put number in the temporary buffer */
     while (i-- > 0 && (precision_bak != 0))
     {
         if (buf < end)
             *buf = tmp[i];
         ++ buf;
     }
 
     while (size-- > 0)
     {
         if (buf < end)
             *buf = ' ';
         ++ buf;
     }
 
     return buf;
 }
 
 #if 1
 /**
  * This function will fill a formatted string to buffer.
  *
  * @param  buf is the buffer to save formatted string.
  *
  * @param  size is the size of buffer.
  *
  * @param  fmt is the format parameters.
  *
  * @param  args is a list of variable parameters.
  *
  * @return The number of characters actually written to buffer.
  */
 RT_WEAK int rt_vsnprintf(char *buf, rt_size_t size, const char *fmt, va_list args)
 {
 #ifdef RT_KPRINTF_USING_LONGLONG
     unsigned long long num;
 #else
     rt_uint32_t num;
 #endif /* RT_KPRINTF_USING_LONGLONG */
     int i, len;
     char *str, *end, c;
     const char *s;
 
     rt_uint8_t base;            /* the base of number */
     rt_uint8_t flags;           /* flags to print number */
     rt_uint8_t qualifier;       /* 'h', 'l', or 'L' for integer fields */
     rt_int32_t field_width;     /* width of output field */
 
 #ifdef RT_PRINTF_PRECISION
     int precision;      /* min. # of digits for integers and max for a string */
 #endif /* RT_PRINTF_PRECISION */
 
     str = buf;
     end = buf + size;
 
     /* Make sure end is always >= buf */
     if (end < buf)
     {
         end  = ((char *) - 1);
         size = end - buf;
     }
 
     for (; *fmt ; ++fmt)
     {
         if (*fmt != '%')
         {
             if (str < end)
                 *str = *fmt;
             ++ str;
             continue;
         }
 
         /* process flags */
         flags = 0;
 
         while (1)
         {
             /* skips the first '%' also */
             ++ fmt;
             if (*fmt == '-') flags |= LEFT;
             else if (*fmt == '+') flags |= PLUS;
             else if (*fmt == ' ') flags |= SPACE;
             else if (*fmt == '#') flags |= SPECIAL;
             else if (*fmt == '0') flags |= ZEROPAD;
             else break;
         }
 
         /* get field width */
         field_width = -1;
         if (_ISDIGIT(*fmt)) field_width = skip_atoi(&fmt);
         else if (*fmt == '*')
         {
             ++ fmt;
             /* it's the next argument */
             field_width = va_arg(args, int);
             if (field_width < 0)
             {
                 field_width = -field_width;
                 flags |= LEFT;
             }
         }
 
 #ifdef RT_PRINTF_PRECISION
         /* get the precision */
         precision = -1;
         if (*fmt == '.')
         {
             ++ fmt;
             if (_ISDIGIT(*fmt)) precision = skip_atoi(&fmt);
             else if (*fmt == '*')
             {
                 ++ fmt;
                 /* it's the next argument */
                 precision = va_arg(args, int);
             }
             if (precision < 0) precision = 0;
         }
 #endif /* RT_PRINTF_PRECISION */
         /* get the conversion qualifier */
         qualifier = 0;
 #ifdef RT_KPRINTF_USING_LONGLONG
         if (*fmt == 'h' || *fmt == 'l' || *fmt == 'L')
 #else
         if (*fmt == 'h' || *fmt == 'l')
 #endif /* RT_KPRINTF_USING_LONGLONG */
         {
             qualifier = *fmt;
             ++ fmt;
 #ifdef RT_KPRINTF_USING_LONGLONG
             if (qualifier == 'l' && *fmt == 'l')
             {
                 qualifier = 'L';
                 ++ fmt;
             }
 #endif /* RT_KPRINTF_USING_LONGLONG */
         }
 
         /* the default base */
         base = 10;
 
         switch (*fmt)
         {
         case 'c':
             if (!(flags & LEFT))
             {
                 while (--field_width > 0)
                 {
                     if (str < end) *str = ' ';
                     ++ str;
                 }
             }
 
             /* get character */
             c = (rt_uint8_t)va_arg(args, int);
             if (str < end) *str = c;
             ++ str;
 
             /* put width */
             while (--field_width > 0)
             {
                 if (str < end) *str = ' ';
                 ++ str;
             }
             continue;
 
         case 's':
             s = va_arg(args, char *);
             if (!s) s = "(NULL)";
 
             len = rt_strlen(s);
 #ifdef RT_PRINTF_PRECISION
             if (precision > 0 && len > precision) len = precision;
 #endif /* RT_PRINTF_PRECISION */
 
             if (!(flags & LEFT))
             {
                 while (len < field_width--)
                 {
                     if (str < end) *str = ' ';
                     ++ str;
                 }
             }
 
             for (i = 0; i < len; ++i)
             {
                 if (str < end) *str = *s;
                 ++ str;
                 ++ s;
             }
 
             while (len < field_width--)
             {
                 if (str < end) *str = ' ';
                 ++ str;
             }
             continue;
 
         case 'p':
             if (field_width == -1)
             {
                 field_width = sizeof(void *) << 1;
                 flags |= ZEROPAD;
             }
 #ifdef RT_PRINTF_PRECISION
             str = print_number(str, end,
                                (long)va_arg(args, void *),
                                16, field_width, precision, flags);
 #else
             str = print_number(str, end,
                                (long)va_arg(args, void *),
                                16, field_width, flags);
 #endif /* RT_PRINTF_PRECISION */
             continue;
 
         case '%':
             if (str < end) *str = '%';
             ++ str;
             continue;
 
         /* integer number formats - set up the flags and "break" */
         case 'o':
             base = 8;
             break;
 
         case 'X':
             flags |= LARGE;
         case 'x':
             base = 16;
             break;
 
         case 'd':
         case 'i':
             flags |= SIGN;
         case 'u':
             break;
 
         default:
             if (str < end) *str = '%';
             ++ str;
 
             if (*fmt)
             {
                 if (str < end) *str = *fmt;
                 ++ str;
             }
             else
             {
                 -- fmt;
             }
             continue;
         }
 
 #ifdef RT_KPRINTF_USING_LONGLONG
         if (qualifier == 'L') num = va_arg(args, long long);
         else if (qualifier == 'l')
 #else
         if (qualifier == 'l')
 #endif /* RT_KPRINTF_USING_LONGLONG */
         {
             num = va_arg(args, rt_uint32_t);
             if (flags & SIGN) num = (rt_int32_t)num;
         }
         else if (qualifier == 'h')
         {
             num = (rt_uint16_t)va_arg(args, rt_int32_t);
             if (flags & SIGN) num = (rt_int16_t)num;
         }
         else
         {
             num = va_arg(args, rt_uint32_t);
             if (flags & SIGN) num = (rt_int32_t)num;
         }
 #ifdef RT_PRINTF_PRECISION
         str = print_number(str, end, num, base, field_width, precision, flags);
 #else
         str = print_number(str, end, num, base, field_width, flags);
 #endif /* RT_PRINTF_PRECISION */
     }
 
     if (size > 0)
     {
         if (str < end) *str = '\0';
         else
         {
             end[-1] = '\0';
         }
     }
 
     /* the trailing null byte doesn't count towards the total
     * ++str;
     */
     return str - buf;
 }
 RTM_EXPORT(rt_vsnprintf);
 #endif
 
 /**
  * This function will fill a formatted string to buffer.
  *
  * @param  buf is the buffer to save formatted string.
  *
  * @param  size is the size of buffer.
  *
  * @param  fmt is the format parameters.
  *
  * @return The number of characters actually written to buffer.
  */
 int rt_snprintf(char *buf, rt_size_t size, const char *fmt, ...)
 {
     rt_int32_t n;
     va_list args;
 
     va_start(args, fmt);
     n = rt_vsnprintf(buf, size, fmt, args);
     va_end(args);
 
     return n;
 }
 RTM_EXPORT(rt_snprintf);
 
 /**
  * This function will fill a formatted string to buffer.
  *
  * @param  buf is the buffer to save formatted string.
  *
  * @param  format is the format parameters.
  *
  * @param  arg_ptr is a list of variable parameters.
  *
  * @return The number of characters actually written to buffer.
  */
 int rt_vsprintf(char *buf, const char *format, va_list arg_ptr)
 {
     return rt_vsnprintf(buf, (rt_size_t) - 1, format, arg_ptr);
 }
 RTM_EXPORT(rt_vsprintf);
 
 /**
  * This function will fill a formatted string to buffer
  *
  * @param  buf the buffer to save formatted string.
  *
  * @param  format is the format parameters.
  *
  * @return The number of characters actually written to buffer.
  */
 int rt_sprintf(char *buf, const char *format, ...)
 {
     rt_int32_t n;
     va_list arg_ptr;
 
     va_start(arg_ptr, format);
     n = rt_vsprintf(buf, format, arg_ptr);
     va_end(arg_ptr);
 
     return n;
 }
 RTM_EXPORT(rt_sprintf);
 
 #ifdef RT_USING_CONSOLE
 
 #ifdef RT_USING_DEVICE
 /**
  * This function returns the device using in console.
  *
  * @return Returns the console device pointer or RT_NULL.
  */
 rt_device_t rt_console_get_device(void)
 {
     return _console_device;
 }
 RTM_EXPORT(rt_console_get_device);
 
 /**
  * This function will set a device as console device.
  * After set a device to console, all output of rt_kprintf will be
  * redirected to this new device.
  *
  * @param  name is the name of new console device.
  *
  * @return the old console device handler on successful, or RT_NULL on failure.
  */
 rt_device_t rt_console_set_device(const char *name)
 {
     rt_device_t new_device, old_device;
 
     /* save old device */
     old_device = _console_device;
 
     /* find new console device */
     new_device = rt_device_find(name);
 
     _console_default_device = rt_device_find(RT_CONSOLE_DEVICE_NAME);
 
     /* check whether it's a same device */
     if (new_device == old_device) return RT_NULL;
 
     if (new_device != RT_NULL)
     {
         if (_console_device != RT_NULL)
         {
             /* close old console device */
             rt_device_close(_console_device);
         }
 
         /* set new console device */
         if (rt_device_open(new_device, RT_DEVICE_OFLAG_RDWR | RT_DEVICE_FLAG_STREAM) == RT_EOK)
         {
             _console_device = new_device;
         }
         else
         {
             _console_device = RT_NULL;
         }
     }
 
     return old_device;
 }
 RTM_EXPORT(rt_console_set_device);
 
 /**
  * This function will set default device as console device.
  * After set, all output of rt_kprintf will be redirected
  * to  default device.
  */
 void rt_console_set_default_device(void)
 {
     /* check whether it's a same device */
     if (_console_default_device == _console_device)
     {
         return;
     }
     else
     {
         if (_console_default_device != RT_NULL)
         {
             if (_console_device != RT_NULL)
             {
                 /* close old console device */
                 rt_device_close(_console_device);
             }
 
             /* set new console device */
             if (rt_device_open(_console_default_device, RT_DEVICE_OFLAG_RDWR | RT_DEVICE_FLAG_STREAM) == RT_EOK)
             {
                 _console_device = _console_default_device;
             }
             else
             {
                 _console_device = RT_NULL;
             }
         }
     }
 }
 RTM_EXPORT(rt_console_set_default_device);
 #endif /* RT_USING_DEVICE */
 
 RT_WEAK void rt_hw_console_output(const char *str)
 {
     /* empty console output */
 }
 RTM_EXPORT(rt_hw_console_output);
 
 /**
  * This function will put string to the console.
  *
  * @param str is the string output to the console.
  */
 void rt_kputs(const char *str)
 {
     if (!str) return;
 
 #ifdef RT_USING_DEVICE
     if (_console_device == RT_NULL)
     {
         rt_hw_console_output(str);
     }
     else
     {
         rt_uint16_t old_flag = _console_device->open_flag;
 
         _console_device->open_flag |= RT_DEVICE_FLAG_STREAM;
         rt_device_write(_console_device, 0, str, rt_strlen(str));
         _console_device->open_flag = old_flag;
     }
 #else
     rt_hw_console_output(str);
 #endif /* RT_USING_DEVICE */
 }
 
 static void rt_kprintf_output(char *buf, rt_size_t len)
 {
 #ifdef RT_USING_DEVICE
     if (_console_device == RT_NULL)
     {
         rt_hw_console_output(buf);
     }
     else
     {
         rt_uint16_t old_flag = _console_device->open_flag;
         _console_device->open_flag |= RT_DEVICE_FLAG_STREAM;
         rt_device_write(_console_device, 0, buf, len);
         _console_device->open_flag = old_flag;
     }
 #else
     rt_hw_console_output(buf);
 #endif /* RT_USING_DEVICE */
 }
 
//  #ifndef CORE_MCU
//  #define PRINTF_RINGBUFFER_SIZE        (2048)
//  #define PRINTF_RB_READ_SIZE           (256)
//  static struct rt_ringbuffer *printf_rb = RT_NULL;
//  static uint8_t printf_read_buff[PRINTF_RB_READ_SIZE] = { 0 };
//  static rt_mutex_t printf_rb_mutex = RT_NULL;
//  static struct rt_event *printf_evt = NULL;
//  #define PRINTF_EVT_ABORT (1 << 4)
 
//  static void printf_thread_entry(void *parameter)
//  {
//      int len = 0;
//      rt_uint32_t evt = 0;
 
//      while (1) {
//          if (printf_rb == RT_NULL || printf_evt == RT_NULL) {
//              break;
//          }
 
//          if (rt_event_recv(printf_evt, PRINTF_EVT_ABORT,
//                                RT_EVENT_FLAG_OR | RT_EVENT_FLAG_CLEAR, RT_WAITING_FOREVER, &evt) == RT_EOK) {
//              if (evt & PRINTF_EVT_ABORT) {
//                  rt_memset(printf_read_buff, 0, PRINTF_RB_READ_SIZE);
//                  len = rt_ringbuffer_get(printf_rb, printf_read_buff, PRINTF_RB_READ_SIZE);
//                  rt_kprintf_output((char *)printf_read_buff, len);
//              }
//          }
//      }
//  }
 
//  int printf_ringbuffer_init(void)
//  {
//      if (printf_rb == RT_NULL) {
//          printf_rb = rt_ringbuffer_create(PRINTF_RINGBUFFER_SIZE);
//          RT_ASSERT(printf_rb);
 
//          printf_rb_mutex = rt_mutex_create("rb_mutex", RT_IPC_FLAG_PRIO);
//          if (printf_rb_mutex == RT_NULL) {
//              return -1;
//          }
 
//          printf_evt = rt_event_create("printf.evt", RT_IPC_FLAG_FIFO);
//          if (printf_evt == RT_NULL) {
//              return -1;
//          }
//          rt_event_control(printf_evt, RT_IPC_CMD_RESET, NULL);
 
//          rt_thread_t tid = RT_NULL;
//          tid = rt_thread_create("pf_rd", printf_thread_entry, RT_NULL, 2048, 15, 5);
//          if (tid != RT_NULL)
//              rt_thread_startup(tid);
//      }
 
//      return 0;
//  }
//  #endif
 
 /**
  * This function will print a formatted string on system console.
  *
  * @param fmt is the format parameters.
  *
  * @return The number of characters actually written to buffer.
  */
 void rt_kprintf(const char *fmt, ...)
 {
     va_list args;
     rt_size_t length;
     static char rt_log_buf[RT_CONSOLEBUF_SIZE];
 
     va_start(args, fmt);
     /* the return value of vsnprintf is the number of bytes that would be
      * written to buffer had if the size of the buffer been sufficiently
      * large excluding the terminating null byte. If the output string
      * would be larger than the rt_log_buf, we have to adjust the output
      * length. */
     length = rt_vsnprintf(rt_log_buf, sizeof(rt_log_buf) - 1, fmt, args);
     if (length > RT_CONSOLEBUF_SIZE - 1)
         length = RT_CONSOLEBUF_SIZE - 1;
 
//  #ifndef CORE_MCU
//      if (printf_rb != RT_NULL) {
//          rt_uint32_t size = rt_ringbuffer_space_len(printf_rb);
//          if (size == 0)
//              return;
 
//          rt_mutex_take(printf_rb_mutex, RT_WAITING_FOREVER);
//          rt_ringbuffer_put(printf_rb, (const uint8_t *)rt_log_buf, length);
//          if (printf_evt) {
//              rt_event_send(printf_evt, PRINTF_EVT_ABORT);
//          }
 
//          rt_mutex_release(printf_rb_mutex);
//          va_end(args);
//          return;
//      }
//  #endif
     rt_kprintf_output(rt_log_buf, length);
 
     va_end(args);
 }
 RTM_EXPORT(rt_kprintf);
 #endif /* RT_USING_CONSOLE */
 
 /* bth 依赖hex dump接口 rt_trace_dump， 等后续log系统起来后再删除掉，并替换成log系统的dump接口 */
 static int rt_format_string(char *buf, size_t size, const char *fmt, ...)
 {
     int len;
     va_list ap;
 
     va_start(ap, fmt);
     len = rt_vsnprintf(&buf[0], size, fmt, ap);
     va_end(ap);
 
     if (len < 0) {
         len = 0;
     } else if (len >= size) {
         len = (size > 1) ? (size - 1) : 0;
     }
     return len;
 }
 
 #ifndef TRACE_DUMP_LEN
 #define TRACE_DUMP_LEN (250)
 char trace_dump_buf[TRACE_DUMP_LEN];
 #endif
 int rt_trace_dump(const char *fmt, unsigned int size, unsigned int count, const void *buffer)
 {
     int len = 0, n = 0, i = 0;
 
     if (!fmt || !buffer) {
         return 0;
     }
 
     switch (size) {
     case sizeof(uint32_t):
         while (i < count && len < sizeof(trace_dump_buf)) {
             len += rt_format_string(&trace_dump_buf[len], sizeof(trace_dump_buf) - len, fmt, *(uint32_t *)((uint32_t *)buffer + i));
             i++;
         }
         break;
     case sizeof(uint16_t):
         while (i < count && len < sizeof(trace_dump_buf)) {
             len += rt_format_string(&trace_dump_buf[len], sizeof(trace_dump_buf) - len, fmt, *(uint16_t *)((uint16_t *)buffer + i));
             i++;
         }
         break;
     case sizeof(uint8_t):
         while (i < count && len < sizeof(trace_dump_buf)) {
             len += rt_format_string(&trace_dump_buf[len], sizeof(trace_dump_buf) - len, fmt, *(uint8_t *)((uint8_t *)buffer + i));
             i++;
         }
         break;
     default:
         return 0;
     }
 
 #ifdef TRACE_CRLF
     if (sizeof(trace_dump_buf) < 2) {
         len = 0;
     } else if (len + 2 > sizeof(trace_dump_buf) && sizeof(trace_dump_buf) >= 2) {
         len = sizeof(trace_dump_buf) - 2;
     }
     if (len + 2 <= sizeof(trace_dump_buf)) {
         trace_dump_buf[len++] = '\r';
         trace_dump_buf[len++] = '\n';
     }
 #else
     if (len + 1 > sizeof(trace_dump_buf) && sizeof(trace_dump_buf) >= 1) {
         len = sizeof(trace_dump_buf) - 1;
     }
     if (len + 1 <= sizeof(trace_dump_buf)) {
         trace_dump_buf[len++] = '\n';
     }
 #endif
 
     //n = hal_trace_output((unsigned char *)trace_dump_buf, len);
     //va_start(args, fmt);
 
 #ifdef RT_USING_DEVICE
     if (_console_device == RT_NULL) {
         rt_hw_console_output(trace_dump_buf);
     } else {
         rt_uint16_t old_flag = _console_device->open_flag;
 
         _console_device->open_flag |= RT_DEVICE_FLAG_STREAM;
         rt_device_write(_console_device, 0, trace_dump_buf, len);
         _console_device->open_flag = old_flag;
     }
 #else
     rt_hw_console_output(trace_dump_buf);
 #endif /* RT_USING_DEVICE */
 
     //va_end(args);
 
     return n;
 }
 
 RTM_EXPORT(rt_trace_dump);
 
 #if 0
 /**
  * This function allocates a memory block, which address is aligned to the
  * specified alignment size.
  *
  * @param  size is the allocated memory block size.
  *
  * @param  align is the alignment size.
  *
  * @return The memory block address was returned successfully, otherwise it was
  *         returned empty RT_NULL.
  */
 void *rt_malloc_align(rt_size_t size, rt_size_t align)
 {
     void *ptr;
     void *align_ptr;
     int uintptr_size;
     rt_size_t align_size;
 
     /* sizeof pointer */
     uintptr_size = sizeof(void*);
     uintptr_size -= 1;
 
     if (!align || !size) {
         return RT_NULL;
     }
 
     /* align the alignment size to uintptr size byte */
     align = ((align + uintptr_size) & ~uintptr_size);
 
     /* get total aligned size */
     align_size = ((size + uintptr_size) & ~uintptr_size) + align;
     /* allocate memory block from heap */
     ptr = rt_malloc(align_size);
     if (ptr != RT_NULL)
     {
         /* the allocated memory block is aligned */
         if (((rt_ubase_t)ptr & (align - 1)) == 0)
         {
             align_ptr = (void *)((rt_ubase_t)ptr + align);
         }
         else
         {
             align_ptr = (void *)(((rt_ubase_t)ptr + (align - 1)) & ~(align - 1));
         }
 
         /* set the pointer before alignment pointer to the real pointer */
         *((rt_ubase_t *)((rt_ubase_t)align_ptr - sizeof(void *))) = (rt_ubase_t)ptr;
 
         ptr = align_ptr;
     }
 
     return ptr;
 }
 RTM_EXPORT(rt_malloc_align);
 
 /**
  * This function release the memory block, which is allocated by
  * rt_malloc_align function and address is aligned.
  *
  * @param ptr is the memory block pointer.
  */
 void rt_free_align(void *ptr)
 {
     void *real_ptr;
 
     /* NULL check */
     if (ptr == RT_NULL) return;
     real_ptr = (void *) * (rt_ubase_t *)((rt_ubase_t)ptr - sizeof(void *));
     rt_free(real_ptr);
 }
 RTM_EXPORT(rt_free_align);
 #endif /* RT_USING_HEAP */
 
 #ifndef RT_USING_CPU_FFS
 #ifdef RT_USING_TINY_FFS
 const rt_uint8_t __lowest_bit_bitmap[] =
 {
     /*  0 - 7  */  0,  1,  2, 27,  3, 24, 28, 32,
     /*  8 - 15 */  4, 17, 25, 31, 29, 12, 32, 14,
     /* 16 - 23 */  5,  8, 18, 32, 26, 23, 32, 16,
     /* 24 - 31 */ 30, 11, 13,  7, 32, 22, 15, 10,
     /* 32 - 36 */  6, 21,  9, 20, 19
 };
 
 /**
  * This function finds the first bit set (beginning with the least significant bit)
  * in value and return the index of that bit.
  *
  * Bits are numbered starting at 1 (the least significant bit).  A return value of
  * zero from any of these functions means that the argument was zero.
  *
  * @return return the index of the first bit set. If value is 0, then this function
  * shall return 0.
  */
 int __rt_ffs(int value)
 {
     return __lowest_bit_bitmap[(rt_uint32_t)(value & (value - 1) ^ value) % 37];
 }
 #else
 const rt_uint8_t __lowest_bit_bitmap[] =
 {
     /* 00 */ 0, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0,
     /* 10 */ 4, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0,
     /* 20 */ 5, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0,
     /* 30 */ 4, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0,
     /* 40 */ 6, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0,
     /* 50 */ 4, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0,
     /* 60 */ 5, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0,
     /* 70 */ 4, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0,
     /* 80 */ 7, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0,
     /* 90 */ 4, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0,
     /* A0 */ 5, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0,
     /* B0 */ 4, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0,
     /* C0 */ 6, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0,
     /* D0 */ 4, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0,
     /* E0 */ 5, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0,
     /* F0 */ 4, 0, 1, 0, 2, 0, 1, 0, 3, 0, 1, 0, 2, 0, 1, 0
 };
 
 /**
  * This function finds the first bit set (beginning with the least significant bit)
  * in value and return the index of that bit.
  *
  * Bits are numbered starting at 1 (the least significant bit).  A return value of
  * zero from any of these functions means that the argument was zero.
  *
  * @return Return the index of the first bit set. If value is 0, then this function
  *         shall return 0.
  */
 int __rt_ffs(int value)
 {
     if (value == 0) return 0;
 
     if (value & 0xff)
         return __lowest_bit_bitmap[value & 0xff] + 1;
 
     if (value & 0xff00)
         return __lowest_bit_bitmap[(value & 0xff00) >> 8] + 9;
 
     if (value & 0xff0000)
         return __lowest_bit_bitmap[(value & 0xff0000) >> 16] + 17;
 
     return __lowest_bit_bitmap[(value & 0xff000000) >> 24] + 25;
 }
 #endif /* RT_USING_TINY_FFS */
 #endif /* RT_USING_CPU_FFS */
 
 #ifndef __on_rt_assert_hook
     #define __on_rt_assert_hook(ex, func, line)         __ON_HOOK_ARGS(rt_assert_hook, (ex, func, line))
 #endif
 
 #ifdef RT_DEBUG
 /* RT_ASSERT(EX)'s hook */
 
 void (*rt_assert_hook)(const char *ex, const char *func, rt_size_t line);
 
 /**
  * This function will set a hook function to RT_ASSERT(EX). It will run when the expression is false.
  *
  * @param hook is the hook function.
  */
 void rt_assert_set_hook(void (*hook)(const char *ex, const char *func, rt_size_t line))
 {
     rt_assert_hook = hook;
 }
 
 /**
  * The RT_ASSERT function.
  *
  * @param ex_string is the assertion condition string.
  *
  * @param func is the function name when assertion.
  *
  * @param line is the file line number when assertion.
  */
 void rt_assert_handler(const char *ex_string, const char *func, rt_size_t line)
 {
     volatile char dummy = 0;
 
     if (rt_assert_hook == RT_NULL)
     {
 #ifdef RT_USING_MODULE
         if (dlmodule_self())
         {
             /* close assertion module */
             dlmodule_exit(-1);
         }
         else
 #endif /*RT_USING_MODULE*/
         {
             rt_kprintf("(%s) assertion failed at function:%s, line number:%d \n", ex_string, func, line);
             while (dummy == 0);
         }
     }
     else
     {
         rt_assert_hook(ex_string, func, line);
     }
 }
 RTM_EXPORT(rt_assert_handler);
 #endif /* RT_DEBUG */
 
 #if !defined (RT_USING_NEWLIB) && defined (RT_USING_MINILIBC) && defined (__GNUC__)
 
 #include <sys/types.h>
 void *memcpy(void *dest, const void *src, size_t n) __attribute__((weak, alias("rt_memcpy")));
 void *memset(void *s, int c, size_t n) __attribute__((weak, alias("rt_memset")));
 void *memmove(void *dest, const void *src, size_t n) __attribute__((weak, alias("rt_memmove")));
 int   memcmp(const void *s1, const void *s2, size_t n) __attribute__((weak, alias("rt_memcmp")));
 
 size_t strlen(const char *s) __attribute__((weak, alias("rt_strlen")));
 char *strstr(const char *s1, const char *s2) __attribute__((weak, alias("rt_strstr")));
 int strcasecmp(const char *a, const char *b) __attribute__((weak, alias("rt_strcasecmp")));
 char *strncpy(char *dest, const char *src, size_t n) __attribute__((weak, alias("rt_strncpy")));
 int strncmp(const char *cs, const char *ct, size_t count) __attribute__((weak, alias("rt_strncmp")));
 #ifdef RT_USING_HEAP
 char *strdup(const char *s) __attribute__((weak, alias("rt_strdup")));
 #endif
 
 #endif
 
 #if defined (__GNUC__)
 #include <ctype.h>
 #include <math.h>
 #include <stdlib.h>
 
 double __wrap_strtod(const char *nptr, char **endptr)
 {
     double x = 0.0;
     int sign = 1;
     int expn = 0;
     int expn2 = 0;
     int n;
     int dot = 0;
     const char *p = nptr;
 
     // skip leading white spaces
     while (isspace(*p))
         p++;
 
     // check sign
     if (*p == '-') {
         sign = -1;
         p++;
     } else if (*p == '+') {
         p++;
     }
 
     // parse digits
     while (isdigit(*p) || (*p == '.' && !dot)) {
         if (*p == '.') {
             dot = 1;
         } else {
             x = x * 10.0 + (*p - '0');
             if (dot)
                 expn--;
         }
         p++;
     }
 
     // parse exponent
     if (toupper(*p) == 'E') {
         p++;
         if (*p == '-') {
             n = -1;
             p++;
         } else if (*p == '+') {
             n = 1;
             p++;
         } else {
             n = 1;
         }
         while (isdigit(*p)) {
             expn2 = expn2 * 10 + (*p - '0');
             p++;
         }
         expn += n * expn2;
     }
 
     if (endptr)
         *endptr = (char*)p;
 
     return sign * x * pow(10.0, expn);
 }
 
 int __wrap_sprintf(char *buf, const char *format, ...)
 {
     rt_int32_t n;
     va_list arg_ptr;
 
     va_start(arg_ptr, format);
     n = rt_vsprintf(buf, format, arg_ptr);
     va_end(arg_ptr);
 
     return n;
 }
 
 // int sprintf(char *buf, const char *format, ...) __attribute__((alias("rt_sprintf")));
 int snprintf(char *buf, rt_size_t size, const char *fmt, ...) __attribute__((weak, alias("rt_snprintf")));
 
 #endif
 
 /**@}*/
 